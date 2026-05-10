//! SHA-256 over the canonical JSON of the input.

use std::sync::Arc;

use async_trait::async_trait;
use kernelflow_core::{
    hash::hash_json, KernelResult, NodeContext, NodeInput, NodeOutput, ResourceRequirements,
    WorkflowNode,
};

use crate::{NodeDescriptor, NodeRegistry, TypeSig};

pub struct Sha256Node;

#[async_trait]
impl WorkflowNode for Sha256Node {
    fn kind(&self) -> &'static str {
        "sha256"
    }
    async fn execute(&self, _ctx: &NodeContext, input: NodeInput) -> KernelResult<NodeOutput> {
        let h = hash_json(&input.payload);
        Ok(NodeOutput {
            value: serde_json::Value::String(h),
            gas_used: 5,
        })
    }
}

pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(
        NodeDescriptor {
            kind: "sha256",
            doc: "SHA-256 of canonical-JSON input. Output: hex string.",
            input: TypeSig::Json,
            output: TypeSig::String,
            requirements: ResourceRequirements::default(),
        },
        Arc::new(|_args| Ok(Arc::new(Sha256Node) as Arc<dyn WorkflowNode>)),
    );
}
