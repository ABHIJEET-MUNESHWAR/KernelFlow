//! Workflow DAG. Generic over the node payload `N` so the same engine can
//! orchestrate WASM modules, native Rust closures, or remote RPC calls.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::error::{KernelError, KernelResult};

pub type NodeId = String;
pub type WorkflowId = uuid::Uuid;

/// Predicate evaluated on a parent node's output to decide whether the
/// edge fires. Kept purposely simple (string DSL) so it round-trips JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EdgeCondition {
    Always,
    /// JSON-pointer (`/result/ok`) compared to a literal value.
    JsonEq { pointer: String, value: serde_json::Value },
}

impl EdgeCondition {
    pub fn evaluate(&self, output: &serde_json::Value) -> bool {
        match self {
            EdgeCondition::Always => true,
            EdgeCondition::JsonEq { pointer, value } => {
                output.pointer(pointer).map(|v| v == value).unwrap_or(false)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub from: NodeId,
    pub to: NodeId,
    pub condition: EdgeCondition,
}

/// Generic DAG. `N` is the node payload (e.g. WASM module bytes, a closure key,
/// etc.). Using a generic instead of `dyn` lets us enforce constraints (e.g.
/// `Send + Sync + Serialize`) at compile time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dag<N> {
    pub id:    WorkflowId,
    pub name:  String,
    pub nodes: BTreeMap<NodeId, N>,
    pub edges: Vec<Edge>,
}

impl<N> Dag<N> {
    /// Topologically sort the DAG. Returns [`KernelError::CycleDetected`] on cycles.
    /// O(V + E).
    pub fn topo_sort(&self) -> KernelResult<Vec<NodeId>> {
        let mut indeg: BTreeMap<NodeId, usize> =
            self.nodes.keys().map(|k| (k.clone(), 0)).collect();
        for e in &self.edges {
            *indeg.entry(e.to.clone()).or_default() += 1;
        }
        let mut q: VecDeque<NodeId> =
            indeg.iter().filter(|(_, &d)| d == 0).map(|(k, _)| k.clone()).collect();
        let mut out = Vec::with_capacity(self.nodes.len());
        while let Some(n) = q.pop_front() {
            out.push(n.clone());
            for e in self.edges.iter().filter(|e| e.from == n) {
                let d = indeg.get_mut(&e.to).unwrap();
                *d -= 1;
                if *d == 0 {
                    q.push_back(e.to.clone());
                }
            }
        }
        if out.len() != self.nodes.len() {
            return Err(KernelError::CycleDetected);
        }
        Ok(out)
    }

    pub fn parents(&self, node: &NodeId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| &e.to == node).collect()
    }

    pub fn children(&self, node: &NodeId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| &e.from == node).collect()
    }
}

/// Fluent builder so the type system enforces "must call `build()`".
pub struct DagBuilder<N> {
    name:  String,
    nodes: BTreeMap<NodeId, N>,
    edges: Vec<Edge>,
}

impl<N> DagBuilder<N> {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), nodes: BTreeMap::new(), edges: Vec::new() }
    }
    pub fn node(mut self, id: impl Into<NodeId>, payload: N) -> Self {
        self.nodes.insert(id.into(), payload);
        self
    }
    pub fn edge(
        mut self,
        from: impl Into<NodeId>,
        to: impl Into<NodeId>,
        cond: EdgeCondition,
    ) -> Self {
        self.edges.push(Edge { from: from.into(), to: to.into(), condition: cond });
        self
    }
    pub fn build(self) -> KernelResult<Dag<N>> {
        let dag = Dag { id: uuid::Uuid::new_v4(), name: self.name, nodes: self.nodes, edges: self.edges };
        // validate references
        let known: BTreeSet<&NodeId> = dag.nodes.keys().collect();
        for e in &dag.edges {
            if !known.contains(&e.from) { return Err(KernelError::NodeNotFound(e.from.clone())); }
            if !known.contains(&e.to)   { return Err(KernelError::NodeNotFound(e.to.clone())); }
        }
        let _ = dag.topo_sort()?; // reject cycles at build time.
        Ok(dag)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topo_orders_simple_chain() {
        let dag = DagBuilder::<u32>::new("t")
            .node("a", 1).node("b", 2).node("c", 3)
            .edge("a", "b", EdgeCondition::Always)
            .edge("b", "c", EdgeCondition::Always)
            .build().unwrap();
        assert_eq!(dag.topo_sort().unwrap(), vec!["a", "b", "c"]);
    }

    #[test]
    fn cycle_is_rejected() {
        let r = DagBuilder::<u32>::new("t")
            .node("a", 1).node("b", 2)
            .edge("a", "b", EdgeCondition::Always)
            .edge("b", "a", EdgeCondition::Always)
            .build();
        assert!(matches!(r, Err(KernelError::CycleDetected)));
    }

    #[test]
    fn condition_json_eq() {
        let c = EdgeCondition::JsonEq { pointer: "/ok".into(), value: serde_json::json!(true) };
        assert!(c.evaluate(&serde_json::json!({ "ok": true })));
        assert!(!c.evaluate(&serde_json::json!({ "ok": false })));
    }
}

