//! Concrete [`TrustPolicy`] implementations.
//!
//! ## HostTrustPolicy
//!
//! [`HostTrustPolicy`] implements the [`TrustPolicy`] trait with two
//! built-in modes:
//!
//! - **Fail-closed** (`fail_closed()`) — every request evaluates to
//!   [`Sandbox`](EffectiveTrustClass::Sandbox), regardless of source.
//!   This is the safe default when the caller wants explicit opt-in.
//!
//! - **User-trusted** (`user_trusted()`) — grants
//!   [`UserTrusted`](EffectiveTrustClass::UserTrusted) to [`LocalUser`]
//!   requests and Sandbox to everything else.

use crate::security::error::TrustResult;
use crate::security::trust::{
    AuthorityCeiling, EffectiveTrustClass, TrustDecision, TrustPolicy, TrustPolicyInput,
    TrustProvenance, TrustSource,
};

// ---------------------------------------------------------------------------
// HostTrustMode
// ---------------------------------------------------------------------------

/// Internal mode selector for [`HostTrustPolicy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostTrustMode {
    /// Deny everything (Sandbox).
    FailClosed,
    /// Grant UserTrusted to local users, Sandbox to others.
    UserTrusted,
}

// ---------------------------------------------------------------------------
// HostTrustPolicy
// ---------------------------------------------------------------------------

/// A simple, host-local trust policy with two modes.
///
/// # Modes
///
/// | Mode              | LocalUser | RemotePeer | SystemProcess | WASMPlugin |
/// |-------------------|-----------|------------|---------------|------------|
/// | `fail_closed()`   | Sandbox   | Sandbox    | Sandbox       | Sandbox    |
/// | `user_trusted()`  | UserTrusted | Sandbox  | Sandbox       | Sandbox    |
///
/// # Example
///
/// ```rust,ignore
/// use k2_core::security::{HostTrustPolicy, TrustPolicy, TrustPolicyInput};
///
/// let policy = HostTrustPolicy::user_trusted();
/// let input = TrustPolicyInput::local_user(vec![]);
/// let decision = policy.evaluate(&input).unwrap();
/// assert!(decision.effective_trust.at_least(&EffectiveTrustClass::UserTrusted));
/// ```
#[derive(Debug, Clone)]
pub struct HostTrustPolicy {
    mode: HostTrustMode,
}

impl HostTrustPolicy {
    /// Create a **fail-closed** policy.
    ///
    /// Every evaluation returns [`Sandbox`](EffectiveTrustClass::Sandbox),
    /// regardless of the source or requested effects.  This is the safest
    /// default and should be used when launching untrusted plugins or
    /// remote peers.
    pub fn fail_closed() -> Self {
        Self {
            mode: HostTrustMode::FailClosed,
        }
    }

    /// Create a **user-trusted** policy.
    ///
    /// Requests originating from [`TrustSource::LocalUser`] receive
    /// [`UserTrusted`](EffectiveTrustClass::UserTrusted) with a
    /// permissive user ceiling.  All other sources fall back to Sandbox.
    pub fn user_trusted() -> Self {
        Self {
            mode: HostTrustMode::UserTrusted,
        }
    }
}

impl TrustPolicy for HostTrustPolicy {
    fn evaluate(&self, input: &TrustPolicyInput) -> TrustResult<TrustDecision> {
        match self.mode {
            HostTrustMode::FailClosed => Ok(TrustDecision {
                effective_trust: EffectiveTrustClass::sandbox(),
                authority_ceiling: AuthorityCeiling::default(),
                provenance: TrustProvenance::new(
                    input.source.clone(),
                    "fail-closed: all requests sandboxed",
                ),
            }),

            HostTrustMode::UserTrusted => {
                let (class, ceiling) = match input.source {
                    TrustSource::LocalUser => (
                        EffectiveTrustClass::user_trusted(),
                        AuthorityCeiling::user(),
                    ),
                    _ => (
                        EffectiveTrustClass::sandbox(),
                        AuthorityCeiling::default(),
                    ),
                };

                let class_label = class.label().to_owned();

                Ok(TrustDecision {
                    effective_trust: class,
                    authority_ceiling: ceiling,
                    provenance: TrustProvenance::new(
                        input.source.clone(),
                        format!(
                            "user-trusted policy: source={} => class={}",
                            input.source, class_label,
                        ),
                    ),
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::trust::EffectKind;

    // -- Fail-closed ---------------------------------------------------------

    #[test]
    fn fail_closed_returns_sandbox_for_local_user() {
        let policy = HostTrustPolicy::fail_closed();
        let input = TrustPolicyInput::local_user(vec![EffectKind::Read]);
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
        assert!(decision.authority_ceiling.allowed_effects.is_empty());
    }

    #[test]
    fn fail_closed_returns_sandbox_for_remote_peer() {
        let policy = HostTrustPolicy::fail_closed();
        let input = TrustPolicyInput::remote_peer(Some("peer".into()), vec![EffectKind::Network]);
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
    }

    #[test]
    fn fail_closed_returns_sandbox_for_system_process() {
        let policy = HostTrustPolicy::fail_closed();
        let input = TrustPolicyInput {
            peer_id: None,
            source: TrustSource::SystemProcess,
            requested_effects: vec![],
        };
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
    }

    #[test]
    fn fail_closed_returns_sandbox_for_wasm_plugin() {
        let policy = HostTrustPolicy::fail_closed();
        let input = TrustPolicyInput {
            peer_id: None,
            source: TrustSource::WASMPlugin,
            requested_effects: vec![],
        };
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
    }

    // -- User-trusted --------------------------------------------------------

    #[test]
    fn user_trusted_returns_user_trusted_for_local_user() {
        let policy = HostTrustPolicy::user_trusted();
        let input = TrustPolicyInput::local_user(vec![EffectKind::Read]);
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(
            decision.effective_trust,
            EffectiveTrustClass::user_trusted()
        );
        // User ceiling allows Read
        assert!(decision.authority_ceiling.allows(EffectKind::Read));
        // User ceiling does NOT allow Admin
        assert!(!decision.authority_ceiling.allows(EffectKind::Admin));
    }

    #[test]
    fn user_trusted_returns_sandbox_for_remote_peer() {
        let policy = HostTrustPolicy::user_trusted();
        let input = TrustPolicyInput::remote_peer(Some("peer".into()), vec![EffectKind::Network]);
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
    }

    #[test]
    fn user_trusted_returns_sandbox_for_wasm_plugin() {
        let policy = HostTrustPolicy::user_trusted();
        let input = TrustPolicyInput {
            peer_id: None,
            source: TrustSource::WASMPlugin,
            requested_effects: vec![EffectKind::Execute],
        };
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
    }

    #[test]
    fn user_trusted_local_user_cannot_admin() {
        // LocalUser gets UserTrusted, but the user ceiling excludes Admin.
        let policy = HostTrustPolicy::user_trusted();
        let input = TrustPolicyInput::local_user(vec![EffectKind::Admin]);
        let decision = policy.evaluate(&input).unwrap();

        assert_eq!(
            decision.effective_trust,
            EffectiveTrustClass::user_trusted()
        );
        assert!(!decision.authority_ceiling.allows(EffectKind::Admin));
    }
}
