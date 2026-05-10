//! `WorkflowNode` trait — every executable unit (WASM, native, RPC) implements it.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::KernelResult;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NodeInput {
    pub workflow_id: uuid::Uuid,
    pub node_id:     String,
    pub payload:     serde_json::Value,
    /// Aggregated parent outputs keyed by parent node id.
    pub parents:     std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeOutput {
    pub value:    serde_json::Value,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct NodeContext {
    pub timeout:     Duration,
    pub max_retries: u32,
    pub trace_id:    String,
}

impl Default for NodeContext {
    fn default() -> Self {
        Self {
            timeout:     Duration::from_secs(30),
            max_retries: 3,
            trace_id:    uuid::Uuid::new_v4().to_string(),
        }
    }
}

/// Object-safe via `async_trait`. Generic constraints (`Send + Sync`) are
/// enforced at compile time so the scheduler can shard tasks across threads.
#[async_trait]
pub trait WorkflowNode: Send + Sync + 'static {
    async fn execute(&self, ctx: &NodeContext, input: NodeInput) -> KernelResult<NodeOutput>;

    /// Stable identifier used for metrics/tracing.
    fn kind(&self) -> &'static str;
}

