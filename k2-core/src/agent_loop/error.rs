//! Error types for the agent loop module.
//!
//! Uses `thiserror` to provide structured, composable error variants
//! covering LLM failures, tool invocation errors, store failures, trust
//! policy denials, serialization issues, and iteration limit enforcement.

use thiserror::Error;

use crate::capabilities::error::CapabilityError;
use crate::llm::error::LlmError;
use crate::security::error::TrustError;
use crate::store::StoreError;

/// Errors that can occur during agent loop execution.
#[derive(Error, Debug)]
pub enum AgentLoopError {
    /// An error from the LLM provider (HTTP, parsing, rate limit, etc.).
    #[error("LLM error: {0}")]
    LlmError(#[from] LlmError),

    /// An error from the tool registry or a tool invocation.
    #[error("Tool error: {0}")]
    ToolError(#[from] CapabilityError),

    /// An error from the checkpoint store (database, serialization, etc.).
    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    /// An error from the trust policy evaluation.
    #[error("Trust error: {0}")]
    TrustError(#[from] TrustError),

    /// Failed to serialize or deserialize loop state.
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// The maximum number of iterations was exceeded.
    #[error("max iterations exceeded: {0}")]
    MaxIterationsExceeded(u32),

    /// A checkpoint was not found when attempting to resume.
    #[error("checkpoint not found: {0}")]
    CheckpointNotFound(String),

    /// The loop was in an unexpected state for the requested operation.
    #[error("invalid loop state: {0}")]
    InvalidState(String),

    /// Catch-all for errors that do not fit the categories above.
    #[error("internal agent loop error: {0}")]
    Internal(String),
}

/// Convenience type alias for results returned by the agent loop module.
pub type AgentLoopResult<T> = Result<T, AgentLoopError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = AgentLoopError::MaxIterationsExceeded(10);
        assert_eq!(err.to_string(), "max iterations exceeded: 10");

        let err = AgentLoopError::CheckpointNotFound("abc-123".to_string());
        assert_eq!(err.to_string(), "checkpoint not found: abc-123");
    }

    #[test]
    fn test_from_llm_error() {
        let llm_err = LlmError::NoContent;
        let err: AgentLoopError = llm_err.into();
        assert!(matches!(err, AgentLoopError::LlmError(_)));
    }

    #[test]
    fn test_from_store_error() {
        let store_err = StoreError::General("db down".to_string());
        let err: AgentLoopError = store_err.into();
        assert!(matches!(err, AgentLoopError::StoreError(_)));
    }

    #[test]
    fn test_from_trust_error() {
        let trust_err = TrustError::PolicyDenied("not allowed".to_string());
        let err: AgentLoopError = trust_err.into();
        assert!(matches!(err, AgentLoopError::TrustError(_)));
    }
}