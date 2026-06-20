//! Approval system for gating tool invocations behind trust policies.
//!
//! # Architecture
//!
//! - **[`request`]** — [`ApprovalRequest`] struct for tool invocation
//!   requests requiring approval.
//! - **[`gate`]** — [`ApprovalGate`] evaluates trust policy and returns
//!   [`Allow`](gate::ApprovalGateResult::Allow),
//!   [`Block`](gate::ApprovalGateResult::Block), or
//!   [`Deny`](gate::ApprovalGateResult::Deny).
//! - **[`resolver`]** — [`ApprovalResolver`] persists approvals and
//!   issues [`CapabilityLease`](lease::CapabilityLease) with fail-closed
//!   semantics.
//! - **[`lease`]** — [`LeaseApproval`], [`CapabilityLease`], and the
//!   [`LeaseState`] state machine.
//! - **[`error`]** — [`ApprovalError`] and [`ApprovalResult`].
//!
//! # Fail-closed guarantee
//!
//! The resolver persists the approval record **before** issuing a lease.
//! If lease creation fails, the approval remains recoverable — the
//! system errs on the side of denying access rather than granting it
//! without an audit trail.

pub mod error;
pub mod gate;
pub mod lease;
pub mod request;
pub mod resolver;

pub use error::{ApprovalError, ApprovalResult};
pub use gate::{ApprovalGate, ApprovalGateResult};
pub use lease::{CapabilityLease, LeaseApproval, LeaseState};
pub use request::ApprovalRequest;
pub use resolver::ApprovalResolver;
