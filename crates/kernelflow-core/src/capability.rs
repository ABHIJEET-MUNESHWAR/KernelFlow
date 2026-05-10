//! Capabilities & resource accounting — mirrors the Fermah Froben matchmaker
//! model. Workflows declare what they need; operators advertise what they
//! offer; the matchmaker reconciles the two and reserves capacity.

use serde::{Deserialize, Serialize};

/// Coarse-grained capability tag. Matchmakers filter operators by these
/// before finer-grained resource matching. New capabilities are additive
/// (`#[non_exhaustive]`) so adding one is not a breaking change.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Capability {
    /// Generic CPU compute (no special hardware).
    GenericCpu,
    /// WASM sandbox host (wasmtime).
    WasmHost,
    /// HTTP egress through the controlled network path.
    HttpEgress,
    /// On-chain RPC (signing / submission).
    OnchainRpc,
    /// GPU witness generation / circuit prover (a la Froben).
    GpuProver { vram_mb: u32 },
    /// Custom string tag for user extensions.
    Custom(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    pub cpu_cores: u32,
    pub ram_mb: u32,
    pub gpu_vram_mb: u32,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceCapacity {
    pub cpu_cores: u32,
    pub ram_mb: u32,
    pub gpu_vram_mb: u32,
    pub capabilities: Vec<Capability>,
}

impl ResourceCapacity {
    /// Does this capacity satisfy the requested requirements?
    pub fn satisfies(&self, req: &ResourceRequirements) -> bool {
        if self.cpu_cores < req.cpu_cores {
            return false;
        }
        if self.ram_mb < req.ram_mb {
            return false;
        }
        if self.gpu_vram_mb < req.gpu_vram_mb {
            return false;
        }
        req.capabilities
            .iter()
            .all(|c| self.capabilities.contains(c))
    }
}

/// In-memory reservation token. Drop = release (RAII).
#[derive(Debug)]
pub struct Reservation {
    pub op_id: String,
    pub req: ResourceRequirements,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn satisfies_matches_capabilities_and_resources() {
        let cap = ResourceCapacity {
            cpu_cores: 8,
            ram_mb: 32_000,
            gpu_vram_mb: 24_000,
            capabilities: vec![
                Capability::GpuProver { vram_mb: 24_000 },
                Capability::HttpEgress,
            ],
        };
        let req = ResourceRequirements {
            cpu_cores: 4,
            ram_mb: 8_000,
            gpu_vram_mb: 16_000,
            capabilities: vec![Capability::HttpEgress],
        };
        assert!(cap.satisfies(&req));

        let too_much = ResourceRequirements {
            cpu_cores: 16,
            ..req.clone()
        };
        assert!(!cap.satisfies(&too_much));
    }
}
