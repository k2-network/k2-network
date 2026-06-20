//! Error types for the capabilities module.
//!
//! Defines `CapabilityError` using `thiserror` for all tool registry and
//! invocation failures. Follows K2's preferred pattern of specific, typed
//! error variants instead of anyhow/panic.

use thiserror::Error;

/// Errors that can occur when working with the tool registry or invoking tools.
#[derive(Error, Debug)]
pub enum CapabilityError {
    /// A tool with the given ID has already been registered.
    #[error("tool already registered: {0}")]
    AlreadyRegistered(String),

    /// No tool with the given ID was found in the registry.
    #[error("tool not found: {0}")]
    NotFound(String),

    /// The caller does not have sufficient trust level to invoke this tool.
    #[error(
        "insufficient trust level: required={required}, caller={caller}"
    )]
    InsufficientTrust {
        /// Minimum trust level the tool requires.
        required: String,
        /// Actual trust level of the invocation context.
        caller: String,
    },

    /// The tool invocation returned an error result.
    #[error("tool invocation returned error: {0}")]
    InvocationFailed(String),

    /// Failed to serialise or deserialise tool input/output.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// The tool's JSON Schema is invalid or malformed.
    #[error("invalid tool schema: {0}")]
    InvalidSchema(String),

    /// The tool's input does not conform to its declared JSON Schema.
    #[error("input validation failed: {0}")]
    ValidationFailed(String),

    /// A lock on the registry could not be acquired (poisoned mutex).
    #[error("registry lock poisoned")]
    LockPoisoned,

    /// Catch-all for errors that do not fit the categories above.
    #[error("internal capability error: {0}")]
    Internal(String),
}

/// Convenience alias for results using `CapabilityError`.
pub type CapabilityResult<T> = Result<T, CapabilityError>;
