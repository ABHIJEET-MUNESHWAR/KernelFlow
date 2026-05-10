//! Operator + delegation types.

use std::collections::HashMap;
use std::sync::Arc;

use kernelflow_core::{KernelError, KernelResult, ResourceCapacity, ResourceRequirements};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

pub type OperatorId    = String;
pub type DelegationId  = uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operator {
    pub id:        OperatorId,
    pub total:     ResourceCapacity,
    pub in_flight: ResourceCapacity,  // resources currently committed
    /// Reputation in `[0.0, 1.0]`. EWMA of recent successful completions.
    pub reputation: f64,
}

impl Operator {
    pub fn new(id: impl Into<OperatorId>, total: ResourceCapacity) -> Self {
        Self {
            id: id.into(),
            in_flight: ResourceCapacity { capabilities: total.capabilities.clone(), ..Default::default() },
            total,
            reputation: 0.5,
        }
    }

    pub fn free(&self) -> ResourceCapacity {
        ResourceCapacity {
            cpu_cores:    self.total.cpu_cores.saturating_sub(self.in_flight.cpu_cores),
            ram_mb:       self.total.ram_mb.saturating_sub(self.in_flight.ram_mb),
            gpu_vram_mb:  self.total.gpu_vram_mb.saturating_sub(self.in_flight.gpu_vram_mb),
            capabilities: self.total.capabilities.clone(),
        }
    }

    pub fn can_take(&self, req: &ResourceRequirements) -> bool {
        self.free().satisfies(req)
    }
}

/// A unit of work the workflow is asking the network to execute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delegation {
    pub id:           DelegationId,
    pub kind:         String,          // node kind (e.g. "circuit_prover")
    pub args:         serde_json::Value,
    pub requirements: ResourceRequirements,
}

#[derive(Debug, Clone)]
pub enum DelegationOutcome {
    Success,
    Failure(String),
    Timeout,
    OperatorDisconnect,
}

/// Thread-safe operator pool. All mutation is `async` behind an `RwLock`
/// so the matchmaker can be shared across the scheduler's worker threads.
#[derive(Default, Clone)]
pub struct OperatorRegistry {
    inner: Arc<RwLock<HashMap<OperatorId, Operator>>>,
}

impl OperatorRegistry {
    pub fn new() -> Self { Self::default() }

    pub async fn register(&self, op: Operator) {
        self.inner.write().await.insert(op.id.clone(), op);
    }

    pub async fn deregister(&self, id: &str) -> Option<Operator> {
        self.inner.write().await.remove(id)
    }

    pub async fn snapshot(&self) -> Vec<Operator> {
        self.inner.read().await.values().cloned().collect()
    }

    /// Reserve capacity. Returns `Err(RateLimited)` if not enough free resources.
    pub async fn reserve(&self, id: &str, req: &ResourceRequirements) -> KernelResult<()> {
        let mut g = self.inner.write().await;
        let op = g.get_mut(id).ok_or_else(|| KernelError::NodeNotFound(id.into()))?;
        if !op.can_take(req) {
            return Err(KernelError::RateLimited);
        }
        op.in_flight.cpu_cores   += req.cpu_cores;
        op.in_flight.ram_mb      += req.ram_mb;
        op.in_flight.gpu_vram_mb += req.gpu_vram_mb;
        Ok(())
    }

    pub async fn release(&self, id: &str, req: &ResourceRequirements) {
        if let Some(op) = self.inner.write().await.get_mut(id) {
            op.in_flight.cpu_cores   = op.in_flight.cpu_cores.saturating_sub(req.cpu_cores);
            op.in_flight.ram_mb      = op.in_flight.ram_mb.saturating_sub(req.ram_mb);
            op.in_flight.gpu_vram_mb = op.in_flight.gpu_vram_mb.saturating_sub(req.gpu_vram_mb);
        }
    }

    /// Update reputation EWMA. `α = 0.2` weights recency without thrashing.
    pub async fn record_outcome(&self, id: &str, outcome: &DelegationOutcome) {
        if let Some(op) = self.inner.write().await.get_mut(id) {
            const ALPHA: f64 = 0.2;
            let sample = match outcome {
                DelegationOutcome::Success => 1.0,
                DelegationOutcome::Failure(_) | DelegationOutcome::Timeout => 0.0,
                DelegationOutcome::OperatorDisconnect => 0.0,
            };
            op.reputation = ALPHA * sample + (1.0 - ALPHA) * op.reputation;
        }
    }
}

