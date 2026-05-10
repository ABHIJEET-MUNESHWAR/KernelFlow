//! Top-level supervisor. Walks the DAG in topological order, executes
//! ready nodes in **parallel** via `futures::stream::FuturesUnordered`,
//! emits `KernelEvent`s, and returns a deterministic attestation hash.

use std::collections::BTreeMap;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::broadcast;

use kernelflow_core::{
    Dag, KernelError, KernelEvent, KernelResult, NodeContext, NodeInput, NodeOutcome,
    WorkflowNode, hash::hash_json,
};

use crate::{actor::NodeActor, resilience::{ResilienceConfig, ResilientExecutor}};

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub resilience:  ResilienceConfig,
    pub event_buffer: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self { Self { resilience: Default::default(), event_buffer: 1024 } }
}

pub struct Scheduler {
    cfg:    SchedulerConfig,
    events: broadcast::Sender<KernelEvent>,
}

#[derive(Clone)]
pub struct SchedulerHandle {
    pub events: broadcast::Sender<KernelEvent>,
}

impl Scheduler {
    pub fn new(cfg: SchedulerConfig) -> Self {
        let (events, _) = broadcast::channel(cfg.event_buffer);
        Self { cfg, events }
    }

    pub fn handle(&self) -> SchedulerHandle { SchedulerHandle { events: self.events.clone() } }

    /// Execute a workflow. `node_factory` provides the executable node for a given
    /// (id, payload). Returning `Arc<dyn WorkflowNode>` lets one scheduler drive
    /// heterogeneous node implementations (registry-based).
    pub async fn run<P, F>(
        &self,
        dag: &Dag<P>,
        node_factory: F,
    ) -> KernelResult<String>
    where
        P: Send + Sync + 'static,
        F: Fn(&str, &P) -> Arc<dyn WorkflowNode>,
    {
        let _ = self.events.send(KernelEvent::WorkflowStarted {
            workflow_id: dag.id, name: dag.name.clone(),
        });

        let exec = Arc::new(ResilientExecutor::new(self.cfg.resilience.clone()));

        // Spawn one actor per node.
        let mut actors: BTreeMap<String, NodeActor> = BTreeMap::new();
        for (id, payload) in &dag.nodes {
            actors.insert(id.clone(), NodeActor::spawn(node_factory(id, payload), exec.clone()));
        }

        // Compute in-degree to know when a node is ready.
        let mut indeg: BTreeMap<String, usize> = dag.nodes.keys().map(|k| (k.clone(), 0)).collect();
        for e in &dag.edges { *indeg.get_mut(&e.to).unwrap() += 1; }

        let mut outputs: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        let mut ready: Vec<String> = indeg.iter().filter(|(_, &d)| d == 0).map(|(k, _)| k.clone()).collect();

        while !ready.is_empty() {
            // Run all ready nodes concurrently — true parallel batch processing.
            let mut inflight = FuturesUnordered::new();
            for nid in ready.drain(..) {
                let parents: BTreeMap<String, serde_json::Value> = dag
                    .parents(&nid).iter()
                    .filter_map(|e| outputs.get(&e.from).cloned().map(|v| (e.from.clone(), v)))
                    .collect();
                let input = NodeInput {
                    workflow_id: dag.id,
                    node_id: nid.clone(),
                    payload: serde_json::Value::Null,
                    parents,
                };
                let actor = actors.get(&nid).unwrap();
                let ev = self.events.clone();
                let _ = ev.send(KernelEvent::NodeStarted { workflow_id: dag.id, node_id: nid.clone(), attempt: 1 });
                let fut = async move {
                    let res = actor.call(input, NodeContext::default()).await;
                    (nid, res)
                };
                inflight.push(fut);
            }

            while let Some((nid, res)) = inflight.next().await {
                match res {
                    Ok(out) => {
                        let outcome = NodeOutcome::Success {
                            output_hash: hash_json(&out.value), gas_used: out.gas_used,
                        };
                        let _ = self.events.send(KernelEvent::NodeCompleted {
                            workflow_id: dag.id, node_id: nid.clone(), outcome,
                        });
                        outputs.insert(nid.clone(), out.value);
                        // unlock children whose edge condition holds
                        for e in dag.children(&nid) {
                            let parent_out = outputs.get(&nid).cloned().unwrap_or(serde_json::Value::Null);
                            if !e.condition.evaluate(&parent_out) { continue; }
                            let d = indeg.get_mut(&e.to).unwrap();
                            *d -= 1;
                            if *d == 0 { ready.push(e.to.clone()); }
                        }
                    }
                    Err(err) => {
                        let _ = self.events.send(KernelEvent::WorkflowFailed {
                            workflow_id: dag.id, reason: err.to_string(),
                        });
                        return Err(err);
                    }
                }
            }
        }

        if outputs.len() != dag.nodes.len() {
            return Err(KernelError::InvalidInput("not all nodes were reached".into()));
        }

        let attestation_hash = hash_json(&outputs);
        let _ = self.events.send(KernelEvent::WorkflowCompleted {
            workflow_id: dag.id, attestation_hash: attestation_hash.clone(),
        });
        Ok(attestation_hash)
    }
}

