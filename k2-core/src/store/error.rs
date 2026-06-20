//! Error types for the store module.

use thiserror::Error;

/// Errors that can occur when interacting with the store.
#[derive(Error, Debug)]
pub enum StoreError {
    /// Database connection error.
    #[error("Database connection error: {0}")]
    Connection(#[from] rusqlite::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Checkpoint not found.
    #[error("Checkpoint not found: {0}")]
    CheckpointNotFound(String),

    /// Approval record not found.
    #[error("Approval record not found: {0}")]
    ApprovalNotFound(String),

    /// Invalid checkpoint ID.
    #[error("Invalid checkpoint ID: {0}")]
    InvalidCheckpointId(String),

    /// Invalid approval ID.
    #[error("Invalid approval ID: {0}")]
    InvalidApprovalId(String),

    /// Database migration error.
    #[error("Migration error: {0}")]
    Migration(String),

    /// General store error.
    #[error("Store error: {0}")]
    General(String),
}
