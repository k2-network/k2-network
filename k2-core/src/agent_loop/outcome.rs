//! Loop outcome types and approval request structures.

use serde::{Deserialize, Serialize};

/// A request for user approval before proceeding with a tool invocation.
///
/// When the trust policy blocks a tool call, the loop produces a
/// [`LoopOutcome::Blocked`] containing this struct. The caller can
/// review the request, grant approval, and then resume the loop from
/// the saved checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique identifier for this approval request.
    pub request_id: String,
    /// The ID of the tool that was blocked.
    pub tool_id: String,
    /// The session this request belongs to.
    pub session_id: String,
    /// The input that was intended for the tool invocation.
    pub input: serde_json::Value,
    /// Human-readable explanation of why approval is needed.
    pub reason: String,
}

impl ApprovalRequest {
    /// Create a new approval request.
    pub fn new(
        tool_id: impl Into<String>,
        session_id: impl Into<String>,
        input: serde_json::Value,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            tool_id: tool_id.into(),
            session_id: session_id.into(),
            input,
            reason: reason.into(),
        }
    }
}

/// The terminal result of an agent loop execution.
///
/// Produced by [`crate::agent_loop::executor::AgentLoopExecutor::tick`]
/// and [`crate::agent_loop::executor::AgentLoopExecutor::resume`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LoopOutcome {
    /// The loop completed successfully — the model stopped without
    /// requesting further tool calls.
    Completed {
        /// The final assistant message content, if any.
        final_content: Option<String>,
        /// Total iterations executed.
        iterations: u32,
    },
    /// The loop is blocked pending user approval for a tool invocation.
    /// A checkpoint has been saved so the loop can be resumed after
    /// approval is granted.
    Blocked(ApprovalRequest),
    /// The loop failed due to an unrecoverable error.
    Failed {
        /// Human-readable error description.
        error: String,
        /// Number of iterations completed before the failure.
        iterations: u32,
    },
    /// The loop reached the maximum iteration count without completing.
    MaxIterationsReached {
        /// The configured maximum.
        max: u32,
        /// The actual number of iterations executed.
        iterations: u32,
    },
}

impl LoopOutcome {
    /// Returns `true` if the loop completed successfully.
    pub fn is_completed(&self) -> bool {
        matches!(self, LoopOutcome::Completed { .. })
    }

    /// Returns `true` if the loop is blocked pending approval.
    pub fn is_blocked(&self) -> bool {
        matches!(self, LoopOutcome::Blocked(_))
    }

    /// Returns `true` if the loop failed.
    pub fn is_failed(&self) -> bool {
        matches!(self, LoopOutcome::Failed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_request_new() {
        let req = ApprovalRequest::new(
            "tool.echo",
            "sess-1",
            serde_json::json!({"key": "value"}),
            "requires user trust",
        );
        assert!(!req.request_id.is_empty());
        assert_eq!(req.tool_id, "tool.echo");
        assert_eq!(req.session_id, "sess-1");
        assert_eq!(req.input, serde_json::json!({"key": "value"}));
        assert_eq!(req.reason, "requires user trust");
    }

    #[test]
    fn test_approval_request_unique_ids() {
        let req1 = ApprovalRequest::new("t", "s", serde_json::Value::Null, "r");
        let req2 = ApprovalRequest::new("t", "s", serde_json::Value::Null, "r");
        assert_ne!(req1.request_id, req2.request_id);
    }

    #[test]
    fn test_loop_outcome_completed() {
        let outcome = LoopOutcome::Completed {
            final_content: Some("done".to_string()),
            iterations: 3,
        };
        assert!(outcome.is_completed());
        assert!(!outcome.is_blocked());
        assert!(!outcome.is_failed());
    }

    #[test]
    fn test_loop_outcome_blocked() {
        let req = ApprovalRequest::new("t", "s", serde_json::Value::Null, "r");
        let outcome = LoopOutcome::Blocked(req);
        assert!(!outcome.is_completed());
        assert!(outcome.is_blocked());
        assert!(!outcome.is_failed());
    }

    #[test]
    fn test_loop_outcome_failed() {
        let outcome = LoopOutcome::Failed {
            error: "boom".to_string(),
            iterations: 2,
        };
        assert!(!outcome.is_completed());
        assert!(!outcome.is_blocked());
        assert!(outcome.is_failed());
    }

    #[test]
    fn test_loop_outcome_max_iterations() {
        let outcome = LoopOutcome::MaxIterationsReached {
            max: 10,
            iterations: 10,
        };
        assert!(!outcome.is_completed());
        assert!(!outcome.is_blocked());
        assert!(!outcome.is_failed());
    }

    #[test]
    fn test_loop_outcome_serde_roundtrip() {
        let outcome = LoopOutcome::Completed {
            final_content: Some("hello".to_string()),
            iterations: 1,
        };
        let json = serde_json::to_string(&outcome).unwrap();
        let back: LoopOutcome = serde_json::from_str(&json).unwrap();
        assert!(back.is_completed());
    }
}