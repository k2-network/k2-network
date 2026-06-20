//! Checkpoint storage types and traits.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::store::error::StoreError;

/// A unique identifier for a checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CheckpointId(pub String);

impl CheckpointId {
    pub fn new() -> Self {
        Uuid::new_v4().to_string().into()
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for CheckpointId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for CheckpointId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for CheckpointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A checkpoint representing the state of an agent loop at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopCheckpoint {
    pub id: CheckpointId,
    pub session_id: String,
    pub stage: String,
    pub state: serde_json::Value,
    pub created_at: u64,
    pub updated_at: u64,
}

impl LoopCheckpoint {
    pub fn new(session_id: String, stage: String, state: serde_json::Value) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: CheckpointId::new(),
            session_id,
            stage,
            state,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Trait for checkpoint storage operations.
pub trait CheckpointStore {
    fn save_checkpoint(&self, checkpoint: &LoopCheckpoint) -> Result<CheckpointId, StoreError>;
    fn load_checkpoint(&self, id: &CheckpointId) -> Result<Option<LoopCheckpoint>, StoreError>;
    fn list_checkpoints(&self, session_id: &str) -> Result<Vec<LoopCheckpoint>, StoreError>;
    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<bool, StoreError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_id_generation() {
        let id1 = CheckpointId::new();
        let id2 = CheckpointId::new();
        assert_ne!(id1.0, id2.0);
        assert_eq!(id1.0, id1.as_str());
    }

    #[test]
    fn test_loop_checkpoint_creation() {
        let checkpoint = LoopCheckpoint::new(
            "session-123".to_string(),
            "planning".to_string(),
            serde_json::json!({"key": "value"}),
        );
        assert_eq!(checkpoint.session_id, "session-123");
        assert_eq!(checkpoint.stage, "planning");
        assert_eq!(checkpoint.created_at, checkpoint.updated_at);
    }
}
