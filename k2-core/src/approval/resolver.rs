use std::sync::Arc;

use crate::approval::error::{ApprovalError, ApprovalResult};
use crate::approval::lease::{CapabilityLease, LeaseApproval};
use crate::store::{ApprovalRecordStore, ApprovalStatus};

/// Resolves approval requests by persisting records and issuing
/// capability leases.
///
/// # Fail-closed semantics
///
/// The resolver **always** persists the approval record to the store
/// **before** issuing the lease. If lease creation fails, the approval
/// record remains in the store in a recoverable state.  This is the
/// **fail-closed** pattern: the system errs on the side of denying
/// access rather than granting it without a paper trail.
///
/// # Thread safety
///
/// The store is wrapped in `Arc` and all synchronous store calls are
/// executed via [`tokio::task::spawn_blocking`] to avoid blocking the
/// async runtime.
pub struct ApprovalResolver<S: ApprovalRecordStore + Send + Sync + 'static> {
    store: Arc<S>,
}

impl<S: ApprovalRecordStore + Send + Sync + 'static> ApprovalResolver<S> {
    /// Create a new resolver backed by the given store.
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    /// Approve a request and issue a capability lease.
    ///
    /// # Fail-closed guarantee
    ///
    /// 1. Load the approval record for `request_id`.
    /// 2. Verify it is still pending.
    /// 3. Mark the record as approved and persist.
    /// 4. Issue a [`CapabilityLease`] from the [`LeaseApproval`].
    ///
    /// If step 4 fails, the approved record **remains persisted** —
    /// the caller can retry lease creation without losing the approval.
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::NotFound`] if the request does not exist.
    /// - [`ApprovalError::AlreadyResolved`] if the request is not pending.
    pub async fn approve_dispatch(
        &self,
        request_id: &str,
        approval: LeaseApproval,
    ) -> ApprovalResult<CapabilityLease> {
        let request_id = request_id.to_string();
        let store = Arc::clone(&self.store);

        // Phase 1: persist the approval record (fail-closed boundary)
        let inner = tokio::task::spawn_blocking(move || -> ApprovalResult<(String, String)> {
            let record = store
                .load_approval_by_request_id(&request_id)?
                .ok_or_else(|| ApprovalError::NotFound(request_id.clone()))?;

            if record.status != ApprovalStatus::Pending {
                return Err(ApprovalError::AlreadyResolved(request_id.clone()));
            }

            let tool_id = record.tool_id.clone();
            let mut updated = record;
            updated.mark_approved();
            store.save_approval(&updated)?;
            Ok((updated.id, tool_id))
        })
        .await
        .map_err(|e| ApprovalError::Internal(format!("spawn_blocking failed: {}", e)))?;

        let (record_id, tool_id) = inner?;

        // Phase 2: issue the lease
        let lease = CapabilityLease::new(&record_id, &tool_id, &approval);
        Ok(lease)
    }

    /// Deny a pending approval request.
    ///
    /// The request is marked as [`ApprovalStatus::Denied`] in the store
    /// and no lease is issued.
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::NotFound`] if the request does not exist.
    /// - [`ApprovalError::AlreadyResolved`] if the request is not pending.
    pub async fn deny(&self, request_id: &str, _reason: String) -> ApprovalResult<()> {
        let request_id = request_id.to_string();
        let store = Arc::clone(&self.store);

        let inner = tokio::task::spawn_blocking(move || -> ApprovalResult<()> {
            let record = store
                .load_approval_by_request_id(&request_id)?
                .ok_or_else(|| ApprovalError::NotFound(request_id.clone()))?;

            if record.status != ApprovalStatus::Pending {
                return Err(ApprovalError::AlreadyResolved(request_id.clone()));
            }

            let mut updated = record;
            updated.mark_denied();
            store.save_approval(&updated)?;
            Ok(())
        })
        .await
        .map_err(|e| ApprovalError::Internal(format!("spawn_blocking failed: {}", e)))?;

        inner
    }

    /// Get the current status of an approval request.
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::NotFound`] if the request does not exist.
    pub async fn get_status(&self, request_id: &str) -> ApprovalResult<ApprovalStatus> {
        let request_id = request_id.to_string();
        let store = Arc::clone(&self.store);

        let inner = tokio::task::spawn_blocking(move || -> ApprovalResult<ApprovalStatus> {
            let record = store
                .load_approval_by_request_id(&request_id)?
                .ok_or_else(|| ApprovalError::NotFound(request_id.clone()))?;
            Ok(record.status)
        })
        .await
        .map_err(|e| ApprovalError::Internal(format!("spawn_blocking failed: {}", e)))?;

        inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::lease::LeaseState;
    use crate::security::trust::{EffectKind, ResourceCeiling};
    use crate::store::{ApprovalRecord, SqliteStore};

    fn make_store() -> Arc<SqliteStore> {
        Arc::new(SqliteStore::in_memory().expect("in-memory store"))
    }

    fn make_approval() -> LeaseApproval {
        LeaseApproval::new(
            "test-approver",
            vec![EffectKind::Read],
            Some(ResourceCeiling::default()),
            None,
            Some(5),
        )
    }

    fn seed_approval(store: &SqliteStore, request_id: &str) -> String {
        let record = ApprovalRecord::new(
            request_id.to_string(),
            "tool-test".to_string(),
            "session-test".to_string(),
        );
        store.save_approval(&record).unwrap();
        record.id
    }

    #[tokio::test]
    async fn approve_creates_lease_with_active_state() {
        let store = make_store();
        let request_id = "req-approve-1";
        seed_approval(&store, request_id);
        let resolver = ApprovalResolver::new(Arc::clone(&store));

        let lease = resolver
            .approve_dispatch(request_id, make_approval())
            .await
            .expect("approve_dispatch should succeed");

        assert_eq!(lease.status, LeaseState::Active);
        assert_eq!(lease.invocations_remaining, Some(5));

        let status = resolver
            .get_status(request_id)
            .await
            .expect("get_status should succeed");
        assert_eq!(status, ApprovalStatus::Approved);
    }

    #[tokio::test]
    async fn deny_sets_status_to_denied() {
        let store = make_store();
        let request_id = "req-deny-1";
        seed_approval(&store, request_id);
        let resolver = ApprovalResolver::new(Arc::clone(&store));

        resolver
            .deny(request_id, "not allowed".to_string())
            .await
            .expect("deny should succeed");

        let status = resolver
            .get_status(request_id)
            .await
            .expect("get_status should succeed");
        assert_eq!(status, ApprovalStatus::Denied);
    }

    #[tokio::test]
    async fn approve_twice_fails_with_already_resolved() {
        let store = make_store();
        let request_id = "req-double-1";
        seed_approval(&store, request_id);
        let resolver = ApprovalResolver::new(Arc::clone(&store));

        resolver
            .approve_dispatch(request_id, make_approval())
            .await
            .expect("first approve should succeed");

        let result = resolver
            .approve_dispatch(request_id, make_approval())
            .await;
        assert!(result.is_err());
        match result {
            Err(ApprovalError::AlreadyResolved(_)) => {}
            other => panic!("expected AlreadyResolved, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn deny_twice_fails_with_already_resolved() {
        let store = make_store();
        let request_id = "req-double-deny";
        seed_approval(&store, request_id);
        let resolver = ApprovalResolver::new(Arc::clone(&store));

        resolver
            .deny(request_id, "nope".to_string())
            .await
            .expect("first deny should succeed");

        let result = resolver
            .deny(request_id, "nope again".to_string())
            .await;
        assert!(result.is_err());
        match result {
            Err(ApprovalError::AlreadyResolved(_)) => {}
            other => panic!("expected AlreadyResolved, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn get_status_returns_not_found_for_missing_request() {
        let store = make_store();
        let resolver = ApprovalResolver::new(store);
        let result = resolver.get_status("nonexistent").await;
        assert!(result.is_err());
        match result {
            Err(ApprovalError::NotFound(_)) => {}
            other => panic!("expected NotFound, got {:?}", other),
        }
    }
}
