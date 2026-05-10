//! Constant: emits a static JSON value. Useful for parameterizing workflows.

use std::sync::Arc;

use async_trait::async_trait;
use kernelflow_core::{KernelResult, NodeContext, NodeInput, NodeOutput, ResourceRequirements, WorkflowNode};

use crate::{NodeDescriptor, NodeRegistry, TypeSig};

pub struct ConstantNode { value: serde_json::Value }

impl ConstantNode { pub fn new(value: serde_json::Value) -> Self { Self { value } } }

#[async_trait]
impl WorkflowNode for ConstantNode {
    fn kind(&self) -> &'static str { "constant" }
    async fn execute(&self, _ctx: &NodeContext, _input: NodeInput) -> KernelResult<NodeOutput> {
        Ok(NodeOutput { value: self.value.clone(), gas_used: 1 })
    }
}

pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(
        NodeDescriptor {
            kind: "constant",
            doc:  "Emits a fixed JSON value.",
            input:  TypeSig::Json,
            output: TypeSig::Json,
            requirements: ResourceRequirements::default(),
        },
        Arc::new(|args| Ok(Arc::new(ConstantNode::new(args.clone())) as Arc<dyn WorkflowNode>)),
    );
}

