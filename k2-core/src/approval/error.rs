//! Error types for the approval module.
//!
//! Defines [`ApprovalError`] using `thiserror` for all approval
//! resolution, lease management, and gate-check failures.

use thiserror::Error;

use crate::store::StoreError;

/// Errors that can occur during approval operations.
///
/// Follows K2's convention of typed, descriptive error variants.
/// All variants implement `Clone` so errors can be propagated
/// across `spawn_blocking` boundaries.
#[derive(Error, Debug)]
pub enum ApprovalError {
    /// An approval request with the given id was not found in the store.
    #[error("approval request not found: {0}")]
    NotFound(String),

    /// The approval request has already been resolved (approved, denied,
    /// or expired) and cannot be modified further.
    #[error("approval request already resolved: {0}")]
    AlreadyResolved(String),

    /// Lease creation failed — the approval record was persisted but
    /// the lease could not be issued.  This is the **fail-closed** path:
    /// the approval is recoverable, but the caller must retry.
    #[error("lease creation failed for approval {approval_id}: {reason}")]
    LeaseCreationFailed {
        /// The approval record id that was persisted.
        approval_id: String,
        /// Human-readable description of the failure.
        reason: String,
    },

    /// A requested lease state transition is invalid from the current
    /// state.
    #[error("invalid lease transition: cannot move from {from} to {to}")]
    InvalidLeaseTransition {
        /// The current state of the lease.
        from: String,
        /// The requested target state.
        to: String,
    },

    /// The lease has expired and cannot be used.
    #[error("lease {lease_id} expired at {expired_at}")]
    LeaseExpired {
        /// The lease identifier.
        lease_id: String,
        /// The Unix timestamp at which the lease expired.
        expired_at: u64,
    },

    /// No invocations remain on the lease.
    #[error("lease {lease_id} has no invocations remaining")]
    LeaseExhausted {
        /// The lease identifier.
        lease_id: String,
    },

    /// The trust policy denied the request outright.
    #[error("trust policy denied: {0}")]
    PolicyDenied(String),

    /// A store-level error occurred (database, serialization, etc.).
    #[error("store error: {0}")]
    Store(#[from] StoreError),

    /// Catch-all for internal errors that do not fit the categories above.
    #[error("internal approval error: {0}")]
    Internal(String),
}

/// Convenience type alias for results returned by the approval module.
pub type ApprovalResult<T> = Result<T, ApprovalError>;
