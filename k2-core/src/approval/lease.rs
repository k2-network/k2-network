use serde::{Deserialize, Serialize};

use crate::approval::error::{ApprovalError, ApprovalResult};
use crate::security::trust::{EffectKind, ResourceCeiling};

/// The lifecycle state of a capability lease.
///
/// # State Machine
///
/// ```text
/// Active ──► Claimed ──► Dispatching ──► Consumed
///   │           │              │
///   └───────────┴──────────────┴────────► Revoked
/// ```
///
/// - [`Active`]: lease is ready to be claimed by an invocation.
/// - [`Claimed`]: an invocation has reserved this lease.
/// - [`Dispatching`]: the invocation is in progress.
/// - [`Consumed`]: the invocation completed; lease is spent.
/// - [`Revoked`]: terminal — the lease was explicitly cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LeaseState {
    Active,
    Claimed,
    Dispatching,
    Consumed,
    Revoked,
}

impl std::fmt::Display for LeaseState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Claimed => write!(f, "claimed"),
            Self::Dispatching => write!(f, "dispatching"),
            Self::Consumed => write!(f, "consumed"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

/// Parameters that define the scope of an approval grant.
///
/// Created by the approver (human or automated policy) and used by the
/// [`ApprovalResolver`](super::resolver::ApprovalResolver) to issue a
/// [`CapabilityLease`].
#[derive(Debug, Clone)]
pub struct LeaseApproval {
    /// Identifier of the principal or policy that issued this approval.
    pub issued_by: String,

    /// The effect kinds permitted under this approval.
    pub allowed_effects: Vec<EffectKind>,

    /// Optional resource bounds; `None` means no ceiling.
    pub resource_ceiling: Option<ResourceCeiling>,

    /// Unix timestamp at which this approval expires; `None` means
    /// no expiry.
    pub expires_at: Option<u64>,

    /// Maximum number of invocations allowed; `None` means unlimited.
    pub max_invocations: Option<u64>,
}

impl LeaseApproval {
    /// Create a new lease approval grant.
    pub fn new(
        issued_by: impl Into<String>,
        allowed_effects: Vec<EffectKind>,
        resource_ceiling: Option<ResourceCeiling>,
        expires_at: Option<u64>,
        max_invocations: Option<u64>,
    ) -> Self {
        Self {
            issued_by: issued_by.into(),
            allowed_effects,
            resource_ceiling,
            expires_at,
            max_invocations,
        }
    }

    /// Returns `true` if this approval has expired relative to `now`.
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at.map_or(false, |exp| now >= exp)
    }
}

/// A time-boxed, invocation-counted lease granting permission to invoke
/// a specific tool.
///
/// Leases follow a strict state machine (see [`LeaseState`]) and can be
/// consumed at most once per invocation.  The lease tracks remaining
/// invocations and expiry independently of the approval that spawned it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityLease {
    /// Unique identifier for this lease.
    pub id: String,

    /// The approval record id that authorised this lease.
    pub approval_id: String,

    /// The tool this lease permits invocation of.
    pub tool_id: String,

    /// Current lifecycle state.
    pub status: LeaseState,

    /// Unix timestamp when the lease was created.
    pub created_at: u64,

    /// Unix timestamp when the lease expires; `None` means no expiry.
    pub expires_at: Option<u64>,

    /// How many invocations remain; `None` means unlimited.
    pub invocations_remaining: Option<u64>,
}

impl CapabilityLease {
    /// Create a new lease from an approval grant.
    pub fn new(
        approval_id: impl Into<String>,
        tool_id: impl Into<String>,
        approval: &LeaseApproval,
    ) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            approval_id: approval_id.into(),
            tool_id: tool_id.into(),
            status: LeaseState::Active,
            created_at: now,
            expires_at: approval.expires_at,
            invocations_remaining: approval.max_invocations,
        }
    }

    /// Returns `true` if the lease has expired relative to `now`.
    pub fn is_expired(&self, now: u64) -> bool {
        self.expires_at.map_or(false, |exp| now >= exp)
    }

    /// Returns `true` if the lease can still be used (active, not
    /// expired, has invocations remaining).
    pub fn can_use(&self, now: u64) -> bool {
        self.status == LeaseState::Active
            && !self.is_expired(now)
            && self.invocations_remaining.map_or(true, |r| r > 0)
    }

    // ── State machine transitions ───────────────────────────────────────

    /// Transition from [`Active`] to [`Claimed`].
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::InvalidLeaseTransition`] if not in `Active`.
    /// - [`ApprovalError::LeaseExpired`] if the lease has expired.
    /// - [`ApprovalError::LeaseExhausted`] if no invocations remain.
    pub fn claim(&mut self, now: u64) -> ApprovalResult<()> {
        if self.status != LeaseState::Active {
            return Err(ApprovalError::InvalidLeaseTransition {
                from: self.status.to_string(),
                to: LeaseState::Claimed.to_string(),
            });
        }
        if self.is_expired(now) {
            return Err(ApprovalError::LeaseExpired {
                lease_id: self.id.clone(),
                expired_at: self.expires_at.unwrap_or(0),
            });
        }
        if let Some(remaining) = self.invocations_remaining {
            if remaining == 0 {
                return Err(ApprovalError::LeaseExhausted {
                    lease_id: self.id.clone(),
                });
            }
        }
        self.status = LeaseState::Claimed;
        Ok(())
    }

    /// Transition from [`Claimed`] to [`Dispatching`].
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::InvalidLeaseTransition`] if not in `Claimed`.
    /// - [`ApprovalError::LeaseExpired`] if the lease has expired.
    pub fn dispatch(&mut self, now: u64) -> ApprovalResult<()> {
        if self.status != LeaseState::Claimed {
            return Err(ApprovalError::InvalidLeaseTransition {
                from: self.status.to_string(),
                to: LeaseState::Dispatching.to_string(),
            });
        }
        if self.is_expired(now) {
            return Err(ApprovalError::LeaseExpired {
                lease_id: self.id.clone(),
                expired_at: self.expires_at.unwrap_or(0),
            });
        }
        self.status = LeaseState::Dispatching;
        Ok(())
    }

    /// Transition from [`Dispatching`] to [`Consumed`] and decrement
    /// the invocation counter.
    ///
    /// # Errors
    ///
    /// - [`ApprovalError::InvalidLeaseTransition`] if not in `Dispatching`.
    pub fn consume(&mut self) -> ApprovalResult<()> {
        if self.status != LeaseState::Dispatching {
            return Err(ApprovalError::InvalidLeaseTransition {
                from: self.status.to_string(),
                to: LeaseState::Consumed.to_string(),
            });
        }
        if let Some(ref mut remaining) = self.invocations_remaining {
            *remaining = remaining.saturating_sub(1);
        }
        self.status = LeaseState::Consumed;
        Ok(())
    }

    /// Transition from any non-terminal state to [`Revoked`].
    ///
    /// Once revoked the lease cannot be used again.
    pub fn revoke(&mut self) -> ApprovalResult<()> {
        if self.status == LeaseState::Consumed || self.status == LeaseState::Revoked {
            return Err(ApprovalError::InvalidLeaseTransition {
                from: self.status.to_string(),
                to: LeaseState::Revoked.to_string(),
            });
        }
        self.status = LeaseState::Revoked;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn make_approval() -> LeaseApproval {
        LeaseApproval::new(
            "admin",
            vec![EffectKind::Read, EffectKind::Write],
            Some(ResourceCeiling::default()),
            None,          // no expiry
            Some(10),      // max 10 invocations
        )
    }

    #[test]
    fn lease_created_in_active_state() {
        let approval = make_approval();
        let lease = CapabilityLease::new("approval-1", "tool-read", &approval);
        assert_eq!(lease.status, LeaseState::Active);
        assert_eq!(lease.invocations_remaining, Some(10));
        assert!(lease.expires_at.is_none());
        assert!(lease.can_use(now()));
    }

    #[test]
    fn lease_state_machine_active_to_consumed() {
        let approval = make_approval();
        let mut lease = CapabilityLease::new("approval-2", "tool-write", &approval);
        let t = now();

        // Active → Claimed
        assert!(lease.claim(t).is_ok());
        assert_eq!(lease.status, LeaseState::Claimed);

        // Claimed → Dispatching
        assert!(lease.dispatch(t).is_ok());
        assert_eq!(lease.status, LeaseState::Dispatching);

        // Dispatching → Consumed
        assert!(lease.consume().is_ok());
        assert_eq!(lease.status, LeaseState::Consumed);
        assert_eq!(lease.invocations_remaining, Some(9));
    }

    #[test]
    fn lease_cannot_skip_states() {
        let approval = make_approval();
        let mut lease = CapabilityLease::new("approval-3", "tool-admin", &approval);
        let t = now();

        // Cannot dispatch from Active
        assert!(lease.dispatch(t).is_err());

        // Cannot consume from Active
        assert!(lease.consume().is_err());
    }

    #[test]
    fn lease_revoke_from_active() {
        let approval = make_approval();
        let mut lease = CapabilityLease::new("approval-4", "tool-net", &approval);
        assert!(lease.revoke().is_ok());
        assert_eq!(lease.status, LeaseState::Revoked);
        assert!(!lease.can_use(now()));
    }

    #[test]
    fn lease_revoke_from_claimed() {
        let approval = make_approval();
        let mut lease = CapabilityLease::new("approval-5", "tool-fs", &approval);
        let t = now();
        lease.claim(t).unwrap();
        assert!(lease.revoke().is_ok());
        assert_eq!(lease.status, LeaseState::Revoked);
    }

    #[test]
    fn lease_cannot_revoke_consumed() {
        let approval = make_approval();
        let mut lease = CapabilityLease::new("approval-6", "tool-exec", &approval);
        let t = now();
        lease.claim(t).unwrap();
        lease.dispatch(t).unwrap();
        lease.consume().unwrap();
        assert!(lease.revoke().is_err());
    }

    #[test]
    fn lease_expired_cannot_be_claimed() {
        let approval = LeaseApproval::new(
            "admin",
            vec![EffectKind::Read],
            None,
            Some(100), // expired long ago
            None,
        );
        let mut lease = CapabilityLease::new("approval-7", "tool-read", &approval);
        let result = lease.claim(now());
        assert!(result.is_err());
        match result {
            Err(ApprovalError::LeaseExpired { .. }) => {}
            other => panic!("expected LeaseExpired, got {:?}", other),
        }
    }

    #[test]
    fn lease_exhausted_cannot_be_claimed() {
        let approval = LeaseApproval::new(
            "admin",
            vec![EffectKind::Read],
            None,
            None,
            Some(0), // zero invocations from the start
        );
        let mut lease = CapabilityLease::new("approval-8", "tool-read", &approval);
        let result = lease.claim(now());
        assert!(result.is_err());
        match result {
            Err(ApprovalError::LeaseExhausted { .. }) => {}
            other => panic!("expected LeaseExhausted, got {:?}", other),
        }
    }

    #[test]
    fn lease_approval_expiry_check() {
        let approval = LeaseApproval::new(
            "admin",
            vec![EffectKind::Read],
            None,
            Some(1000),
            None,
        );
        assert!(!approval.is_expired(500));
        assert!(approval.is_expired(1000));
        assert!(approval.is_expired(2000));

        let no_expiry = LeaseApproval::new("admin", vec![], None, None, None);
        assert!(!no_expiry.is_expired(u64::MAX));
    }
}
