//! Canonical error type. Uses `thiserror` and is `#[non_exhaustive]` so
//! variants can be added without breaking SemVer for downstream crates.

use thiserror::Error;

pub type KernelResult<T> = Result<T, KernelError>;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum KernelError {
    #[error("workflow `{0}` not found")]
    WorkflowNotFound(String),

    #[error("node `{0}` not found")]
    NodeNotFound(String),

    #[error("cycle detected in workflow DAG")]
    CycleDetected,

    #[error("node execution timed out after {0:?}")]
    Timeout(std::time::Duration),

    #[error("sandbox error: {0}")]
    Sandbox(String),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("rate limited")]
    RateLimited,

    #[error("circuit open")]
    CircuitOpen,

    #[error("attestation error: {0}")]
    Attestation(String),

    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    Other(#[from] anyhow_compat::AnyError),
}

/// Thin wrapper so we don't pull in `anyhow` here, but still allow `?` from
/// downstream code that wants type-erased errors.
pub mod anyhow_compat {
    #[derive(Debug, thiserror::Error)]
    #[error("{0}")]
    pub struct AnyError(pub Box<dyn std::error::Error + Send + Sync>);
}
