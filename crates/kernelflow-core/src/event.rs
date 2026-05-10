//! Domain events. The whole system is event-driven: scheduler, storage,
//! attestation, and p2p subscribe to these via a tokio broadcast channel.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KernelEvent {
    WorkflowStarted {
        workflow_id: uuid::Uuid,
        name: String,
    },
    NodeStarted {
        workflow_id: uuid::Uuid,
        node_id: String,
        attempt: u32,
    },
    NodeCompleted {
        workflow_id: uuid::Uuid,
        node_id: String,
        outcome: NodeOutcome,
    },
    WorkflowCompleted {
        workflow_id: uuid::Uuid,
        attestation_hash: String,
    },
    WorkflowFailed {
        workflow_id: uuid::Uuid,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeOutcome {
    Success { output_hash: String, gas_used: u64 },
    Failure { error: String },
    Skipped { reason: String },
}
