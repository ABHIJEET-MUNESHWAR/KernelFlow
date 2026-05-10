//! # kernelflow-scheduler
//!
//! Actor-model, cooperative scheduler. One [`NodeActor`] per workflow node,
//! coordinated by a [`Scheduler`] supervisor. Communication is exclusively
//! through `tokio::mpsc` channels — no shared mutable state.
//!
//! Resilience layers (composed bottom-up around every node execution):
//!   1. **Timeout**           via `tokio::time::timeout`
//!   2. **Retry w/ backoff**  via `backoff::ExponentialBackoff`
//!   3. **Circuit breaker**   via `failsafe::Config`
//!   4. **Rate limiting**     via `governor::RateLimiter`
//!
//! Emits [`KernelEvent`]s on a `broadcast::Sender` consumed by storage,
//! attestation, and the GraphQL subscription layer (event-driven μsvc).

pub mod actor;
pub mod resilience;
pub mod scheduler;

pub use actor::NodeActor;
pub use resilience::{ResilienceConfig, ResilientExecutor};
pub use scheduler::{Scheduler, SchedulerConfig, SchedulerHandle};

