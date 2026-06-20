//! Error types for the security / trust policy module.
//!
//! Defines [`TrustError`] using `thiserror` for all trust evaluation,
//! authority ceiling, and invalidation-bus failures. Follows K2's
//! convention of typed, descriptive error variants.

use thiserror::Error;

/// Errors that can occur during trust policy evaluation, authority
/// enforcement, or trust-change invalidation.
#[derive(Error, Debug, Clone)]
pub enum TrustError {
    /// The trust policy rejected the evaluation — the caller is not
    /// authorised for the requested effects.
    #[error("trust policy denied: {0}")]
    PolicyDenied(String),

    /// An invalid transition between trust classes was attempted
    /// (e.g. attempting to downgrade below Sandbox).
    #[error("invalid trust-class transition: from {from} to {to}")]
    InvalidClassTransition {
        /// The trust class before the attempted transition.
        from: String,
        /// The trust class that was requested.
        to: String,
    },

    /// The requested effect exceeds the caller's authority ceiling.
    #[error("authority ceiling exceeded for effect '{effect}'")]
    AuthorityCeilingExceeded {
        /// The effect kind that was denied.
        effect: String,
    },

    /// A trust invalidation operation failed (e.g. no subscribers
    /// attached to the broadcast channel).
    #[error("trust invalidation failed: {0}")]
    InvalidationFailed(String),

    /// Broadcast channel has no active receivers — the invalidation
    /// message could not be delivered to anyone.
    #[error("no subscribers for trust-change broadcast")]
    NoSubscribers,

    /// Catch-all for internal errors that do not fit the categories above.
    #[error("internal trust error: {0}")]
    Internal(String),
}

/// Convenience type alias for results returned by the security module.
pub type TrustResult<T> = Result<T, TrustError>;
