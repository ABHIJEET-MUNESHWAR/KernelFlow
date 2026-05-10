//! Threshold: emits `{ "ok": bool }` if numeric input crosses `threshold`.
//! Combined with a [`crate::MedianNode`] this reproduces the Fermah blog's
//! canonical "did ETH cross $3,000" example.

use std::sync::Arc;

use async_trait::async_trait;
use kernelflow_core::{
    KernelError, KernelResult, NodeContext, NodeInput, NodeOutput, ResourceRequirements,
    WorkflowNode,
};
use serde::Deserialize;

use crate::{NodeDescriptor, NodeRegistry, TypeSig};

#[derive(Debug, Deserialize)]
struct Args {
    threshold: f64,
    op: Op,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Op {
    Gt,
    Gte,
    Lt,
    Lte,
    Eq,
}

pub struct ThresholdNode {
    threshold: f64,
    op: Op,
}

impl ThresholdNode {
    pub fn gt(threshold: f64) -> Self {
        Self {
            threshold,
            op: Op::Gt,
        }
    }
}

#[async_trait]
impl WorkflowNode for ThresholdNode {
    fn kind(&self) -> &'static str {
        "threshold"
    }
    async fn execute(&self, _ctx: &NodeContext, input: NodeInput) -> KernelResult<NodeOutput> {
        // Use first parent's numeric output, or fall back to payload.
        let n: f64 = input
            .parents
            .values()
            .next()
            .and_then(|v| v.as_f64())
            .or_else(|| input.payload.as_f64())
            .ok_or_else(|| KernelError::InvalidInput("threshold needs numeric input".into()))?;
        let ok = match self.op {
            Op::Gt => n > self.threshold,
            Op::Gte => n >= self.threshold,
            Op::Lt => n < self.threshold,
            Op::Lte => n <= self.threshold,
            Op::Eq => (n - self.threshold).abs() < f64::EPSILON,
        };
        Ok(NodeOutput {
            value: serde_json::json!({ "ok": ok, "value": n }),
            gas_used: 1,
        })
    }
}

pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(
        NodeDescriptor {
            kind: "threshold",
            doc: "Compares numeric input vs threshold. Args: { threshold, op: gt|gte|lt|lte|eq }",
            input: TypeSig::Number,
            output: TypeSig::Object,
            requirements: ResourceRequirements::default(),
        },
        Arc::new(|args| {
            let a: Args = serde_json::from_value(args.clone())
                .map_err(|e| KernelError::InvalidInput(e.to_string()))?;
            Ok(Arc::new(ThresholdNode {
                threshold: a.threshold,
                op: a.op,
            }) as Arc<dyn WorkflowNode>)
        }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn gt_fires() {
        let n = ThresholdNode::gt(3000.0);
        let mut parents = std::collections::BTreeMap::new();
        parents.insert("p".into(), serde_json::json!(3050.5));
        let out = n
            .execute(
                &NodeContext::default(),
                NodeInput {
                    parents,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(out.value["ok"], serde_json::json!(true));
    }
}
