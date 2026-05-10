//! # kernelflow-matchmaker
//!
//! The **Froben pattern**: a registry of heterogeneous [`Operator`]s + a
//! [`Matchmaker`] that routes a [`Delegation`] to the best-fit operator,
//! reserves capacity for the duration of the task, releases it on
//! completion / timeout / disconnect.
//!
//! - **Capability filter**: operator must declare every requested capability.
//! - **Resource filter**: operator must have free CPU/RAM/VRAM ≥ request.
//! - **Reputation rank**: ties are randomized so newcomers can earn a track record.
//! - **Kill-and-retry**: disconnects terminate in-flight tasks immediately
//!   (no zombie reservations — the bug-trail Fermah explicitly mentions).
//!
//! `Matchmaker` is a trait so future strategies (price-bid, locality-aware,
//! BFT-quorum) can plug in without touching call sites (Open/Closed).

pub mod operator;
pub mod strategy;

pub use operator::{Delegation, DelegationId, DelegationOutcome, Operator, OperatorId, OperatorRegistry};
pub use strategy::{Matchmaker, ReputationMatchmaker};

