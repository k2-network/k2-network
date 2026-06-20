use serde::{Deserialize, Serialize};

use crate::capabilities::context::TrustLevel;
use crate::store::ApprovalStatus;

/// A request to invoke a tool that may require approval.
///
/// Created when a caller with insufficient trust attempts to invoke
/// a tool. The request is evaluated by [`ApprovalGate`](super::gate::ApprovalGate)
/// and resolved by [`ApprovalResolver`](super::resolver::ApprovalResolver).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique identifier for this approval request.
    pub id: String,

    /// Correlated request identifier (matches the original invocation).
    pub request_id: String,

    /// The tool being requested.
    pub tool_id: String,

    /// The session in which this request was made.
    pub session_id: String,

    /// The tool input payload (arguments).
    pub input: serde_json::Value,

    /// The minimum trust level required to invoke this tool.
    pub trust_level_required: TrustLevel,

    /// Unix timestamp when the request was created.
    pub requested_at: u64,

    /// Current resolution status.
    pub status: ApprovalStatus,
}

impl ApprovalRequest {
    /// Create a new pending approval request.
    ///
    /// The `id` is auto-generated via UUID v4. The status is set to
    /// [`ApprovalStatus::Pending`] and `requested_at` is the current
    /// Unix timestamp.
    pub fn new(
        request_id: impl Into<String>,
        tool_id: impl Into<String>,
        session_id: impl Into<String>,
        input: serde_json::Value,
        trust_level_required: TrustLevel,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: request_id.into(),
            tool_id: tool_id.into(),
            session_id: session_id.into(),
            input,
            trust_level_required,
            requested_at: now,
            status: ApprovalStatus::Pending,
        }
    }

    /// Returns `true` if this request has not yet been resolved.
    pub fn is_pending(&self) -> bool {
        self.status == ApprovalStatus::Pending
    }

    /// Mark the request as approved.
    pub fn mark_approved(&mut self) {
        self.status = ApprovalStatus::Approved;
    }

    /// Mark the request as denied.
    pub fn mark_denied(&mut self) {
        self.status = ApprovalStatus::Denied;
    }

    /// Mark the request as expired.
    pub fn mark_expired(&mut self) {
        self.status = ApprovalStatus::Expired;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_request_creation() {
        let req = ApprovalRequest::new(
            "req-1",
            "tool-read",
            "session-1",
            serde_json::json!({"path": "/tmp/test"}),
            TrustLevel::FirstParty,
        );
        assert_eq!(req.request_id, "req-1");
        assert_eq!(req.tool_id, "tool-read");
        assert_eq!(req.session_id, "session-1");
        assert_eq!(req.trust_level_required, TrustLevel::FirstParty);
        assert_eq!(req.status, ApprovalStatus::Pending);
        assert!(req.is_pending());
        assert!(!req.id.is_empty());
    }

    #[test]
    fn approval_request_status_transitions() {
        let mut req = ApprovalRequest::new(
            "req-2",
            "tool-write",
            "session-2",
            serde_json::json!({}),
            TrustLevel::UserTrusted,
        );

        assert!(req.is_pending());

        req.mark_approved();
        assert_eq!(req.status, ApprovalStatus::Approved);
        assert!(!req.is_pending());

        let mut req2 = ApprovalRequest::new(
            "req-3",
            "tool-admin",
            "session-3",
            serde_json::json!({}),
            TrustLevel::System,
        );
        req2.mark_denied();
        assert_eq!(req2.status, ApprovalStatus::Denied);

        let mut req3 = ApprovalRequest::new(
            "req-4",
            "tool-net",
            "session-4",
            serde_json::json!({}),
            TrustLevel::Sandbox,
        );
        req3.mark_expired();
        assert_eq!(req3.status, ApprovalStatus::Expired);
    }
}
