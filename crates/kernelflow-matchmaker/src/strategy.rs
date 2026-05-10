//! Matchmaking strategies. New strategies drop in via the [`Matchmaker`] trait.

use async_trait::async_trait;
use kernelflow_core::{KernelError, KernelResult, ResourceRequirements};
use rand::seq::SliceRandom;

use crate::operator::{OperatorId, OperatorRegistry};

#[async_trait]
pub trait Matchmaker: Send + Sync {
    async fn select(
        &self,
        registry: &OperatorRegistry,
        req: &ResourceRequirements,
    ) -> KernelResult<OperatorId>;
}

/// Reputation-ranked, ties broken by uniform random sampling so newcomers
/// (and ties) get fair shots — exactly the policy described in the
/// Fermah Froben blog post.
pub struct ReputationMatchmaker;

#[async_trait]
impl Matchmaker for ReputationMatchmaker {
    async fn select(
        &self,
        registry: &OperatorRegistry,
        req: &ResourceRequirements,
    ) -> KernelResult<OperatorId> {
        let ops = registry.snapshot().await;
        let mut candidates: Vec<_> = ops.into_iter().filter(|o| o.can_take(req)).collect();
        if candidates.is_empty() {
            return Err(KernelError::Network(
                "no operator can satisfy requirements".into(),
            ));
        }
        // Sort descending by reputation, then random tie-break inside top group.
        candidates.sort_by(|a, b| {
            b.reputation
                .partial_cmp(&a.reputation)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let top_score = candidates[0].reputation;
        let top: Vec<_> = candidates
            .iter()
            .filter(|o| (o.reputation - top_score).abs() < 1e-9)
            .collect();
        let chosen = top
            .choose(&mut rand::thread_rng())
            .ok_or_else(|| KernelError::Network("empty top set".into()))?;
        Ok(chosen.id.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operator::Operator;
    use kernelflow_core::{Capability, ResourceCapacity};

    fn cap(caps: Vec<Capability>, vram: u32) -> ResourceCapacity {
        ResourceCapacity {
            cpu_cores: 8,
            ram_mb: 32_000,
            gpu_vram_mb: vram,
            capabilities: caps,
        }
    }

    #[tokio::test]
    async fn picks_capability_match_with_highest_reputation() {
        let reg = OperatorRegistry::new();
        let mut a = Operator::new("a", cap(vec![Capability::GenericCpu], 0));
        a.reputation = 0.2;
        let mut b = Operator::new(
            "b",
            cap(vec![Capability::GpuProver { vram_mb: 24_000 }], 24_000),
        );
        b.reputation = 0.9;
        let mut c = Operator::new(
            "c",
            cap(vec![Capability::GpuProver { vram_mb: 24_000 }], 24_000),
        );
        c.reputation = 0.4;
        reg.register(a).await;
        reg.register(b).await;
        reg.register(c).await;
        let req = ResourceRequirements {
            gpu_vram_mb: 16_000,
            capabilities: vec![Capability::GpuProver { vram_mb: 24_000 }],
            ..Default::default()
        };
        let picked = ReputationMatchmaker.select(&reg, &req).await.unwrap();
        assert_eq!(picked, "b");
    }

    #[tokio::test]
    async fn reservation_blocks_doublebooking() {
        let reg = OperatorRegistry::new();
        reg.register(Operator::new("solo", cap(vec![Capability::GenericCpu], 0)))
            .await;
        let req = ResourceRequirements {
            cpu_cores: 8,
            capabilities: vec![Capability::GenericCpu],
            ..Default::default()
        };
        reg.reserve("solo", &req).await.unwrap();
        // 2nd reservation can't fit.
        assert!(matches!(
            reg.reserve("solo", &req).await,
            Err(KernelError::RateLimited)
        ));
        reg.release("solo", &req).await;
        reg.reserve("solo", &req).await.unwrap();
    }
}
