//! Median: takes the parents' numeric outputs, returns the median.
//! Mirrors the canonical Fermah example "fetch five prices, take the median".

use std::sync::Arc;

use async_trait::async_trait;
use kernelflow_core::{
    KernelError, KernelResult, NodeContext, NodeInput, NodeOutput, ResourceRequirements,
    WorkflowNode,
};

use crate::{NodeDescriptor, NodeRegistry, TypeSig};

pub struct MedianNode;

#[async_trait]
impl WorkflowNode for MedianNode {
    fn kind(&self) -> &'static str {
        "median"
    }
    async fn execute(&self, _ctx: &NodeContext, input: NodeInput) -> KernelResult<NodeOutput> {
        let mut nums: Vec<f64> = input
            .parents
            .values()
            .filter_map(|v| {
                v.as_f64()
                    .or_else(|| v.get("value").and_then(|x| x.as_f64()))
            })
            .collect();
        if nums.is_empty() {
            return Err(KernelError::InvalidInput(
                "no numeric parents for median".into(),
            ));
        }
        nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = nums.len();
        let m = if n % 2 == 1 {
            nums[n / 2]
        } else {
            (nums[n / 2 - 1] + nums[n / 2]) / 2.0
        };
        Ok(NodeOutput {
            value: serde_json::json!(m),
            gas_used: 3,
        })
    }
}

pub(crate) fn register(r: &mut NodeRegistry) {
    r.register(
        NodeDescriptor {
            kind: "median",
            doc: "Median of all numeric parent outputs.",
            input: TypeSig::Number,
            output: TypeSig::Number,
            requirements: ResourceRequirements::default(),
        },
        Arc::new(|_args| Ok(Arc::new(MedianNode) as Arc<dyn WorkflowNode>)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[tokio::test]
    async fn median_of_three() {
        let mut parents = BTreeMap::new();
        parents.insert("a".into(), serde_json::json!(1));
        parents.insert("b".into(), serde_json::json!(5));
        parents.insert("c".into(), serde_json::json!(3));
        let input = NodeInput {
            parents,
            ..Default::default()
        };
        let out = MedianNode
            .execute(&NodeContext::default(), input)
            .await
            .unwrap();
        assert_eq!(out.value, serde_json::json!(3.0));
    }
}
