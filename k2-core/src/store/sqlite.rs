//! SQLite-backed store implementation.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};

use crate::store::approval::{ApprovalRecord, ApprovalRecordStore, ApprovalStatus};
use crate::store::checkpoint::{CheckpointId, CheckpointStore, LoopCheckpoint};
use crate::store::error::StoreError;
use crate::store::migrations;

pub struct SqliteStore {
    conn: Mutex<Connection>,
}

impl SqliteStore {
    pub fn new(db_path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(db_path)?;
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        migrations::run_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }
}

impl CheckpointStore for SqliteStore {
    fn save_checkpoint(&self, checkpoint: &LoopCheckpoint) -> Result<CheckpointId, StoreError> {
        let conn = self.conn.lock().unwrap();
        let state_json = serde_json::to_string(&checkpoint.state)?;
        conn.execute(
            "INSERT OR REPLACE INTO checkpoints (id, session_id, stage, state, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                checkpoint.id.as_str(),
                checkpoint.session_id,
                checkpoint.stage,
                state_json,
                checkpoint.created_at as i64,
                checkpoint.updated_at as i64,
            ],
        )?;
        Ok(checkpoint.id.clone())
    }

    fn load_checkpoint(&self, id: &CheckpointId) -> Result<Option<LoopCheckpoint>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, stage, state, created_at, updated_at FROM checkpoints WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id.as_str()])?;
        if let Some(row) = rows.next()? {
            let state_json: String = row.get(3)?;
            let state: serde_json::Value = serde_json::from_str(&state_json)?;
            Ok(Some(LoopCheckpoint {
                id: CheckpointId(row.get(0)?),
                session_id: row.get(1)?,
                stage: row.get(2)?,
                state,
                created_at: row.get::<_, i64>(4)? as u64,
                updated_at: row.get::<_, i64>(5)? as u64,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_checkpoints(&self, session_id: &str) -> Result<Vec<LoopCheckpoint>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, stage, state, created_at, updated_at FROM checkpoints WHERE session_id = ?1 ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map(params![session_id], |row| {
            let state_json: String = row.get(3)?;
            let state: serde_json::Value = serde_json::from_str(&state_json).unwrap_or_default();
            Ok(LoopCheckpoint {
                id: CheckpointId(row.get(0)?),
                session_id: row.get(1)?,
                stage: row.get(2)?,
                state,
                created_at: row.get::<_, i64>(4)? as u64,
                updated_at: row.get::<_, i64>(5)? as u64,
            })
        })?;
        let mut checkpoints = Vec::new();
        for row in rows {
            checkpoints.push(row?);
        }
        Ok(checkpoints)
    }

    fn delete_checkpoint(&self, id: &CheckpointId) -> Result<bool, StoreError> {
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM checkpoints WHERE id = ?1", params![id.as_str()])?;
        Ok(affected > 0)
    }
}

impl ApprovalRecordStore for SqliteStore {
    fn save_approval(&self, record: &ApprovalRecord) -> Result<String, StoreError> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO approvals (id, request_id, tool_id, session_id, status, requested_at, resolved_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.id,
                record.request_id,
                record.tool_id,
                record.session_id,
                record.status.to_string(),
                record.requested_at as i64,
                record.resolved_at.map(|t| t as i64),
            ],
        )?;
        Ok(record.id.clone())
    }

    fn load_approval(&self, id: &str) -> Result<Option<ApprovalRecord>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, request_id, tool_id, session_id, status, requested_at, resolved_at FROM approvals WHERE id = ?1",
        )?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let status_str: String = row.get(4)?;
            let status: ApprovalStatus = status_str.parse().unwrap_or(ApprovalStatus::Pending);
            Ok(Some(ApprovalRecord {
                id: row.get(0)?,
                request_id: row.get(1)?,
                tool_id: row.get(2)?,
                session_id: row.get(3)?,
                status,
                requested_at: row.get::<_, i64>(5)? as u64,
                resolved_at: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
            }))
        } else {
            Ok(None)
        }
    }

    fn load_approval_by_request_id(&self, request_id: &str) -> Result<Option<ApprovalRecord>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, request_id, tool_id, session_id, status, requested_at, resolved_at FROM approvals WHERE request_id = ?1",
        )?;
        let mut rows = stmt.query(params![request_id])?;
        if let Some(row) = rows.next()? {
            let status_str: String = row.get(4)?;
            let status: ApprovalStatus = status_str.parse().unwrap_or(ApprovalStatus::Pending);
            Ok(Some(ApprovalRecord {
                id: row.get(0)?,
                request_id: row.get(1)?,
                tool_id: row.get(2)?,
                session_id: row.get(3)?,
                status,
                requested_at: row.get::<_, i64>(5)? as u64,
                resolved_at: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
            }))
        } else {
            Ok(None)
        }
    }

    fn list_pending_approvals(&self) -> Result<Vec<ApprovalRecord>, StoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, request_id, tool_id, session_id, status, requested_at, resolved_at FROM approvals WHERE status = 'pending' ORDER BY requested_at ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            let status_str: String = row.get(4)?;
            let status: ApprovalStatus = status_str.parse().unwrap_or(ApprovalStatus::Pending);
            Ok(ApprovalRecord {
                id: row.get(0)?,
                request_id: row.get(1)?,
                tool_id: row.get(2)?,
                session_id: row.get(3)?,
                status,
                requested_at: row.get::<_, i64>(5)? as u64,
                resolved_at: row.get::<_, Option<i64>>(6)?.map(|t| t as u64),
            })
        })?;
        let mut approvals = Vec::new();
        for row in rows {
            approvals.push(row?);
        }
        Ok(approvals)
    }

    fn update_approval_status(&self, id: &str, status: ApprovalStatus) -> Result<bool, StoreError> {
        let conn = self.conn.lock().unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let affected = conn.execute(
            "UPDATE approvals SET status = ?1, resolved_at = ?2 WHERE id = ?3",
            params![status.to_string(), now, id],
        )?;
        Ok(affected > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::checkpoint::CheckpointStore;
    use crate::store::approval::ApprovalRecordStore;
    use std::env::temp_dir;
    use std::path::PathBuf;

    fn temp_db() -> PathBuf {
        temp_dir().join(format!("k2-test-{}.db", uuid::Uuid::new_v4()))
    }

    #[test]
    fn test_save_and_load_checkpoint() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        let checkpoint = LoopCheckpoint::new(
            "session-1".to_string(),
            "planning".to_string(),
            serde_json::json!({"key": "value"}),
        );
        let id = store.save_checkpoint(&checkpoint).unwrap();
        let loaded = store.load_checkpoint(&id).unwrap().unwrap();
        assert_eq!(loaded.session_id, "session-1");
        assert_eq!(loaded.stage, "planning");
        assert_eq!(loaded.state, serde_json::json!({"key": "value"}));
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_list_checkpoints_by_session() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        for i in 0..3 {
            let checkpoint = LoopCheckpoint::new(
                "session-list".to_string(),
                format!("stage-{}", i),
                serde_json::json!({"n": i}),
            );
            store.save_checkpoint(&checkpoint).unwrap();
        }
        let checkpoints = store.list_checkpoints("session-list").unwrap();
        assert_eq!(checkpoints.len(), 3);
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_delete_checkpoint() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        let checkpoint = LoopCheckpoint::new(
            "session-del".to_string(),
            "delete-me".to_string(),
            serde_json::json!({"deleted": true}),
        );
        let id = store.save_checkpoint(&checkpoint).unwrap();
        assert!(store.delete_checkpoint(&id).unwrap());
        assert!(store.load_checkpoint(&id).unwrap().is_none());
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_save_and_load_approval() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        let record = ApprovalRecord::new(
            "req-1".to_string(),
            "tool-1".to_string(),
            "session-1".to_string(),
        );
        let id = store.save_approval(&record).unwrap();
        let loaded = store.load_approval(&id).unwrap().unwrap();
        assert_eq!(loaded.request_id, "req-1");
        assert_eq!(loaded.tool_id, "tool-1");
        assert_eq!(loaded.status, ApprovalStatus::Pending);
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_update_approval_status() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        let record = ApprovalRecord::new(
            "req-upd".to_string(),
            "tool-upd".to_string(),
            "session-upd".to_string(),
        );
        let id = store.save_approval(&record).unwrap();
        assert!(store.update_approval_status(&id, ApprovalStatus::Approved).unwrap());
        let loaded = store.load_approval(&id).unwrap().unwrap();
        assert_eq!(loaded.status, ApprovalStatus::Approved);
        assert!(loaded.resolved_at.is_some());
        std::fs::remove_file(&db_path).ok();
    }

    #[test]
    fn test_list_pending_approvals() {
        let db_path = temp_db();
        let store = SqliteStore::new(&db_path).unwrap();
        let record1 = ApprovalRecord::new("req-p1".to_string(), "tool-p".to_string(), "session-p".to_string());
        let record2 = ApprovalRecord::new("req-p2".to_string(), "tool-p".to_string(), "session-p".to_string());
        store.save_approval(&record1).unwrap();
        store.save_approval(&record2).unwrap();
        let pending = store.list_pending_approvals().unwrap();
        assert_eq!(pending.len(), 2);
        std::fs::remove_file(&db_path).ok();
    }
}
