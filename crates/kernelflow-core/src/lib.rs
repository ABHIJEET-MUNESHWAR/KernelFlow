//! # kernelflow-core
//!
//! Core, dependency-light primitives shared by every other crate:
//! workflow DAG types, the [`WorkflowNode`] trait, the canonical
//! [`KernelError`] type and the [`KernelEvent`] enum used by the
//! event-driven micro-services architecture.
//!
//! Designed around SOLID:
//! * **S**: each module has one responsibility (dag / node / error / event).
//! * **O**: new node kinds plug in via the `WorkflowNode` trait.
//! * **L**: any `WorkflowNode` impl is substitutable in the scheduler.
//! * **I**: trait surface is minimal & async.
//! * **D**: scheduler/storage depend on these abstractions, not concretes.

pub mod capability;
pub mod dag;
pub mod error;
pub mod event;
pub mod hash;
pub mod node;

pub use capability::{Capability, Reservation, ResourceCapacity, ResourceRequirements};
pub use dag::{Dag, DagBuilder, EdgeCondition, NodeId, WorkflowId};
pub use error::{KernelError, KernelResult};
pub use event::{KernelEvent, NodeOutcome};
pub use node::{NodeContext, NodeInput, NodeOutput, WorkflowNode};
