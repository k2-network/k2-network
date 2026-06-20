//! Approval record storage types and traits.

use serde::{Deserialize, Serialize};

use crate::store::error::StoreError;

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

impl std::fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApprovalStatus::Pending => write!(f, "pending"),
            ApprovalStatus::Approved => write!(f, "approved"),
            ApprovalStatus::Denied => write!(f, "denied"),
            ApprovalStatus::Expired => write!(f, "expired"),
        }
    }
}

impl std::str::FromStr for ApprovalStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pending" => Ok(ApprovalStatus::Pending),
            "approved" => Ok(ApprovalStatus::Approved),
            "denied" => Ok(ApprovalStatus::Denied),
            "expired" => Ok(ApprovalStatus::Expired),
            _ => Err(format!("Unknown approval status: {}", s)),
        }
    }
}

/// An approval record for a tool invocation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub id: String,
    pub request_id: String,
    pub tool_id: String,
    pub session_id: String,
    pub status: ApprovalStatus,
    pub requested_at: u64,
    pub resolved_at: Option<u64>,
}

impl ApprovalRecord {
    pub fn new(request_id: String, tool_id: String, session_id: String) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request_id,
            tool_id,
            session_id,
            status: ApprovalStatus::Pending,
            requested_at: now,
            resolved_at: None,
        }
    }

    pub fn mark_approved(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.status = ApprovalStatus::Approved;
        self.resolved_at = Some(now);
    }

    pub fn mark_denied(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.status = ApprovalStatus::Denied;
        self.resolved_at = Some(now);
    }

    pub fn mark_expired(&mut self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.status = ApprovalStatus::Expired;
        self.resolved_at = Some(now);
    }
}

/// Trait for approval record storage operations.
pub trait ApprovalRecordStore {
    fn save_approval(&self, record: &ApprovalRecord) -> Result<String, StoreError>;
    fn load_approval(&self, id: &str) -> Result<Option<ApprovalRecord>, StoreError>;
    fn list_pending_approvals(&self) -> Result<Vec<ApprovalRecord>, StoreError>;
    fn update_approval_status(&self, id: &str, status: ApprovalStatus) -> Result<bool, StoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_status_display() {
        assert_eq!(ApprovalStatus::Pending.to_string(), "pending");
        assert_eq!(ApprovalStatus::Approved.to_string(), "approved");
        assert_eq!(ApprovalStatus::Denied.to_string(), "denied");
        assert_eq!(ApprovalStatus::Expired.to_string(), "expired");
    }

    #[test]
    fn test_approval_status_from_str() {
        assert_eq!("pending".parse::<ApprovalStatus>().unwrap(), ApprovalStatus::Pending);
        assert_eq!("APPROVED".parse::<ApprovalStatus>().unwrap(), ApprovalStatus::Approved);
    }

    #[test]
    fn test_approval_record_creation() {
        let record = ApprovalRecord::new(
            "req-123".to_string(),
            "tool-456".to_string(),
            "session-789".to_string(),
        );
        assert_eq!(record.status, ApprovalStatus::Pending);
        assert!(record.resolved_at.is_none());
    }

    #[test]
    fn test_approval_record_mark_approved() {
        let mut record = ApprovalRecord::new(
            "req-123".to_string(),
            "tool-456".to_string(),
            "session-789".to_string(),
        );
        record.mark_approved();
        assert_eq!(record.status, ApprovalStatus::Approved);
        assert!(record.resolved_at.is_some());
    }
}
