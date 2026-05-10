//! # kernelflow-nodes
//!
//! The **verified node library**. Mirrors Fermah Kernel's "fixed library of
//! verified primitives" — workflows are composed from these, never from
//! arbitrary user code. This is what makes AI-driven composition safe.
//!
//! Every node here:
//! * implements `kernelflow_core::WorkflowNode`
//! * declares its [`Capability`] requirements
//! * is registered in the [`NodeRegistry`] for composition-time validation
//!
//! ## The registry pattern (Open/Closed)
//! Adding a new node = `register!(MyNode)` + impl. No scheduler change needed.

use std::collections::HashMap;
use std::sync::Arc;

use kernelflow_core::{
    KernelError, KernelResult, ResourceRequirements, WorkflowNode,
};
use serde::{Deserialize, Serialize};

pub mod constant;
pub mod hash_node;
pub mod http;
pub mod median;
pub mod threshold;

pub use constant::ConstantNode;
pub use hash_node::Sha256Node;
pub use http::HttpFetchNode;
pub use median::MedianNode;
pub use threshold::ThresholdNode;

/// Type descriptor used for compile-time-ish validation of workflow wiring.
/// (Closer to runtime today; can be promoted to `const fn` later.)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum TypeSig {
    Number,
    String,
    Bytes,
    Boolean,
    Json,
    Array(Box<TypeSig>),
    Object,
}

/// Static description of a node kind. The registry consults this to validate
/// that the upstream `output` shape is assignable to the downstream `input`
/// shape *before* execution.
#[derive(Debug, Clone)]
pub struct NodeDescriptor {
    pub kind:         &'static str,
    pub doc:          &'static str,
    pub input:        TypeSig,
    pub output:       TypeSig,
    pub requirements: ResourceRequirements,
}

/// A factory closure that hydrates a runtime [`WorkflowNode`] from JSON args.
pub type NodeFactory =
    Arc<dyn (Fn(&serde_json::Value) -> KernelResult<Arc<dyn WorkflowNode>>) + Send + Sync>;

/// The fixed library of verified primitives. Lookup is O(1).
#[derive(Default, Clone)]
pub struct NodeRegistry {
    descriptors: HashMap<&'static str, NodeDescriptor>,
    factories:   HashMap<&'static str, NodeFactory>,
}

impl NodeRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, desc: NodeDescriptor, factory: NodeFactory) {
        self.descriptors.insert(desc.kind, desc.clone());
        self.factories.insert(desc.kind, factory);
    }

    pub fn describe(&self, kind: &str) -> Option<&NodeDescriptor> { self.descriptors.get(kind) }
    pub fn kinds(&self) -> impl Iterator<Item = &&'static str> { self.descriptors.keys() }

    pub fn instantiate(&self, kind: &str, args: &serde_json::Value) -> KernelResult<Arc<dyn WorkflowNode>> {
        let f = self.factories.get(kind)
            .ok_or_else(|| KernelError::NodeNotFound(kind.to_string()))?;
        f(args)
    }

    /// Validate that `producer.output` is assignable to `consumer.input`.
    /// Runs at composition time, before anyone burns CPU executing.
    pub fn validate_edge(&self, producer_kind: &str, consumer_kind: &str) -> KernelResult<()> {
        let p = self.describe(producer_kind).ok_or_else(|| KernelError::NodeNotFound(producer_kind.into()))?;
        let c = self.describe(consumer_kind).ok_or_else(|| KernelError::NodeNotFound(consumer_kind.into()))?;
        if !assignable(&p.output, &c.input) {
            return Err(KernelError::InvalidInput(format!(
                "type mismatch: {producer_kind}: {:?} → {consumer_kind}: {:?}", p.output, c.input
            )));
        }
        Ok(())
    }

    /// Pre-loaded with the standard library.
    pub fn with_stdlib() -> Self {
        let mut r = Self::new();
        constant::register(&mut r);
        hash_node::register(&mut r);
        http::register(&mut r);
        median::register(&mut r);
        threshold::register(&mut r);
        r
    }
}

fn assignable(from: &TypeSig, to: &TypeSig) -> bool {
    use TypeSig::*;
    matches!((from, to),
        (Json, _) | (_, Json) // Json is the universal "any"
    ) || from == to
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stdlib_loads_and_validates() {
        let r = NodeRegistry::with_stdlib();
        assert!(r.describe("median").is_some());
        // median outputs Number; threshold expects Number.
        r.validate_edge("median", "threshold").unwrap();
    }

    #[test]
    fn type_mismatch_caught_at_composition() {
        let r = NodeRegistry::with_stdlib();
        // sha256 outputs String; median expects Number.
        let err = r.validate_edge("sha256", "median").unwrap_err();
        assert!(matches!(err, KernelError::InvalidInput(_)));
    }
}

// ---- helper: trivial async-friendly noop NodeContext clone shim ----
#[allow(dead_code)]
async fn _ensure_send(_n: Arc<dyn WorkflowNode>) {}

