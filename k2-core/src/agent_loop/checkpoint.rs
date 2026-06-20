//! Checkpoint integration for the agent loop.
//!
//! Provides helpers to serialise [`LoopExecutionState`] into a
//! [`LoopCheckpoint`] and persist it via a [`CheckpointStore`].
//! Because the checkpoint store is synchronous (rusqlite-backed), all
//! store calls are wrapped in `tokio::task::spawn_blocking` when
//! invoked from async contexts.

use std::sync::Arc;

use crate::store::{CheckpointId, CheckpointStore, LoopCheckpoint, StoreError};

use super::error::{AgentLoopError, AgentLoopResult};
use super::state::{LoopExecutionState, LoopStage};

/// Serialise loop execution state into a JSON value suitable for
/// storage in a [`LoopCheckpoint`].
pub fn serialize_state(state: &LoopExecutionState) -> AgentLoopResult<serde_json::Value> {
    serde_json::to_value(state).map_err(AgentLoopError::from)
}

/// Deserialise loop execution state from a JSON value stored in a
/// [`LoopCheckpoint`].
pub fn deserialize_state(value: &serde_json::Value) -> AgentLoopResult<LoopExecutionState> {
    serde_json::from_value(value.clone()).map_err(AgentLoopError::from)
}

/// Build a [`LoopCheckpoint`] from the current execution state.
pub fn build_checkpoint(state: &LoopExecutionState) -> AgentLoopResult<LoopCheckpoint> {
    let serialized = serialize_state(state)?;
    Ok(LoopCheckpoint::new(
        state.session_id.clone(),
        state.current_stage.as_str().to_string(),
        serialized,
    ))
}

/// Save a checkpoint synchronously.
///
/// This is the raw synchronous call — callers in async contexts should
/// use [`save_checkpoint_async`] instead.
pub fn save_checkpoint_sync(
    store: &dyn CheckpointStore,
    state: &LoopExecutionState,
) -> AgentLoopResult<CheckpointId> {
    let checkpoint = build_checkpoint(state)?;
    let id = store.save_checkpoint(&checkpoint).map_err(AgentLoopError::from)?;
    Ok(id)
}

/// Save a checkpoint asynchronously, wrapping the synchronous store
/// call in `tokio::task::spawn_blocking`.
pub async fn save_checkpoint_async(
    store: Arc<dyn CheckpointStore>,
    state: LoopExecutionState,
) -> AgentLoopResult<CheckpointId> {
    let checkpoint = build_checkpoint(&state)?;
    let store_clone = Arc::clone(&store);
    tokio::task::spawn_blocking(move || {
        store_clone.save_checkpoint(&checkpoint).map_err(AgentLoopError::from)
    })
    .await
    .map_err(|e| AgentLoopError::Internal(format!("checkpoint task panicked: {e}")))?
}

/// Load a checkpoint asynchronously and deserialise its state.
pub async fn load_checkpoint_async(
    store: Arc<dyn CheckpointStore>,
    checkpoint_id: &CheckpointId,
) -> AgentLoopResult<LoopExecutionState> {
    let id = checkpoint_id.clone();
    let store_clone = Arc::clone(&store);
    let checkpoint = tokio::task::spawn_blocking(move || {
        store_clone.load_checkpoint(&id)
    })
    .await
    .map_err(|e| AgentLoopError::Internal(format!("checkpoint load task panicked: {e}")))?
    .map_err(AgentLoopError::from)?;

    let checkpoint = checkpoint.ok_or_else(|| {
        AgentLoopError::CheckpointNotFound(checkpoint_id.to_string())
    })?;

    deserialize_state(&checkpoint.state)
}

/// List checkpoints for a session asynchronously.
pub async fn list_checkpoints_async(
    store: Arc<dyn CheckpointStore>,
    session_id: String,
) -> AgentLoopResult<Vec<LoopCheckpoint>> {
    let store_clone = Arc::clone(&store);
    let checkpoints = tokio::task::spawn_blocking(move || {
        store_clone.list_checkpoints(&session_id)
    })
    .await
    .map_err(|e| AgentLoopError::Internal(format!("list checkpoints task panicked: {e}")))?
    .map_err(AgentLoopError::from)?;

    Ok(checkpoints)
}

/// Delete a checkpoint asynchronously.
pub async fn delete_checkpoint_async(
    store: Arc<dyn CheckpointStore>,
    checkpoint_id: CheckpointId,
) -> AgentLoopResult<bool> {
    let store_clone = Arc::clone(&store);
    let deleted = tokio::task::spawn_blocking(move || {
        store_clone.delete_checkpoint(&checkpoint_id)
    })
    .await
    .map_err(|e| AgentLoopError::Internal(format!("delete checkpoint task panicked: {e}")))?
    .map_err(AgentLoopError::from)?;

    Ok(deleted)
}

/// Restore the stage on a deserialised state, clamping to a valid
/// pipeline stage. This handles forward-compatible deserialisation
/// where unknown stage strings map to [`LoopStage::Input`].
pub fn restore_stage(stage_str: &str) -> LoopStage {
    match stage_str {
        "input" => LoopStage::Input,
        "model" => LoopStage::Model,
        "capability" => LoopStage::Capability,
        "stop" => LoopStage::Stop,
        "completed" => LoopStage::Completed,
        _ => LoopStage::Input,
    }
}

// ---------------------------------------------------------------------------
// In-memory checkpoint store for testing
// ---------------------------------------------------------------------------

use std::collections::HashMap;
use std::sync::Mutex;

/// A simple in-memory [`CheckpointStore`] implementation for tests.
pub struct InMemoryCheckpointStore {
    checkpoints: Mutex<HashMap<String, LoopCheckpoint>>,
}

impl InMemoryCheckpointStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self {
            checkpoints: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryCheckpointStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CheckpointStore for InMemoryCheckpointStore {
    fn save_checkpoint(&self, checkpoint: &LoopCheckpoint) -> Result<CheckpointId, StoreError> {
        let id = checkpoint.id.clone();
        self.checkpoints
            .lock()
            .map_err(|_| StoreError::General("lock poisoned".to_string()))?
            .insert(id.0.clone(), checkpoint.clone());
        Ok(id)
    }

    fn load_checkpoint(&self, id: &CheckpointId) -> Result<Option<LoopCheckpoint>, StoreError> {
        Ok(self
            .checkpoints
            .lock()
            .map_err(|_| StoreError::General("lock poisoned".to_string()))?
            .get(id.as_str())
            .cloned())
    }

    fn list_checkpoints(&self, session_id: &str) -> Result<Vec<LoopCheckpoint>, StoreError> {
        Ok(self
            .checkpoints
            .lock()
            .map_err(|_| StoreError::General("lock poisoned".to_string()))?
            .values()
            .filter(|cp| cp.session_id == session_id)
            .cloned()
            .collect())
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<bool, StoreError> {
        Ok(self
            .checkpoints
            .lock()
            .map_err(|_| StoreError::General("lock poisoned".to_string()))?
            .remove(id.as_str())
            .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_state() {
        let mut state = LoopExecutionState::new("sess-1");
        state.push_message(crate::llm::provider::LlmMessage::user("hello"));
        state.increment_iteration();

        let json = serialize_state(&state).unwrap();
        let restored = deserialize_state(&json).unwrap();

        assert_eq!(restored.session_id, "sess-1");
        assert_eq!(restored.iteration_count, 1);
        assert_eq!(restored.messages.len(), 1);
    }

    #[test]
    fn test_build_checkpoint() {
        let state = LoopExecutionState::new("sess-1");
        let checkpoint = build_checkpoint(&state).unwrap();
        assert_eq!(checkpoint.session_id, "sess-1");
        assert_eq!(checkpoint.stage, "input");
    }

    #[test]
    fn test_save_load_sync() {
        let store = InMemoryCheckpointStore::new();
        let state = LoopExecutionState::new("sess-1");

        let id = save_checkpoint_sync(&store, &state).unwrap();
        let loaded = store.load_checkpoint(&id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().session_id, "sess-1");
    }

    #[tokio::test]
    async fn test_save_load_async() {
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let state = LoopExecutionState::new("sess-async");

        let id = save_checkpoint_async(Arc::clone(&store), state).await.unwrap();
        let restored = load_checkpoint_async(Arc::clone(&store), &id).await.unwrap();
        assert_eq!(restored.session_id, "sess-async");
    }

    #[tokio::test]
    async fn test_load_nonexistent_checkpoint() {
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let fake_id = CheckpointId::from_string("does-not-exist".to_string());
        let result = load_checkpoint_async(store, &fake_id).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            AgentLoopError::CheckpointNotFound(_)
        ));
    }

    #[tokio::test]
    async fn test_list_checkpoints_async() {
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let state1 = LoopExecutionState::new("sess-list");
        let state2 = LoopExecutionState::new("sess-list");
        let state3 = LoopExecutionState::new("other-sess");

        save_checkpoint_async(Arc::clone(&store), state1).await.unwrap();
        save_checkpoint_async(Arc::clone(&store), state2).await.unwrap();
        save_checkpoint_async(Arc::clone(&store), state3).await.unwrap();

        let list = list_checkpoints_async(Arc::clone(&store), "sess-list".to_string())
            .await
            .unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn test_delete_checkpoint_async() {
        let store: Arc<dyn CheckpointStore> = Arc::new(InMemoryCheckpointStore::new());
        let state = LoopExecutionState::new("sess-del");
        let id = save_checkpoint_async(Arc::clone(&store), state).await.unwrap();

        let deleted = delete_checkpoint_async(Arc::clone(&store), id.clone()).await.unwrap();
        assert!(deleted);

        let deleted_again = delete_checkpoint_async(Arc::clone(&store), id).await.unwrap();
        assert!(!deleted_again);
    }

    #[test]
    fn test_restore_stage() {
        assert_eq!(restore_stage("input"), LoopStage::Input);
        assert_eq!(restore_stage("model"), LoopStage::Model);
        assert_eq!(restore_stage("capability"), LoopStage::Capability);
        assert_eq!(restore_stage("stop"), LoopStage::Stop);
        assert_eq!(restore_stage("completed"), LoopStage::Completed);
        assert_eq!(restore_stage("unknown"), LoopStage::Input);
    }
}