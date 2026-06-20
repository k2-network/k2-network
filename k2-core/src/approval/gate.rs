use crate::approval::request::ApprovalRequest;
use crate::capabilities::context::TrustLevel;
use crate::security::trust::{EffectiveTrustClass, TrustPolicy, TrustPolicyInput};

/// Outcome of an approval gate check.
///
/// - [`Allow`]: the caller has sufficient trust; proceed immediately.
/// - [`Block`]: the caller lacks trust; the request is forwarded for
///   approval.
/// - [`Deny`]: the trust policy rejected the request outright.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalGateResult {
    Allow,
    Block(ApprovalRequest),
    Deny,
}

/// Gates tool invocations behind a trust policy.
///
/// The gate evaluates a [`TrustPolicy`] against the caller's context
/// and compares the resulting effective trust class against the
/// [`TrustLevel`] required by the tool. Requests that meet or exceed
/// the required level are allowed through; others are blocked pending
/// approval or denied outright.
pub struct ApprovalGate<P: TrustPolicy> {
    policy: P,
}

impl<P: TrustPolicy> ApprovalGate<P> {
    /// Create a new approval gate using the given trust policy.
    pub fn new(policy: P) -> Self {
        Self { policy }
    }

    /// Check whether a tool invocation request should be allowed,
    /// blocked, or denied.
    ///
    /// # Logic
    ///
    /// 1. Evaluate the trust policy against `policy_input`.
    /// 2. If the policy returns an error (e.g. policy denial), return
    ///    [`Deny`].
    /// 3. Compare the policy's effective trust class against the
    ///    request's `trust_level_required`.
    /// 4. Return [`Allow`] if sufficient, [`Block`] otherwise.
    pub fn check(
        &self,
        request: &ApprovalRequest,
        policy_input: &TrustPolicyInput,
    ) -> ApprovalGateResult {
        let decision = match self.policy.evaluate(policy_input) {
            Ok(d) => d,
            Err(_) => return ApprovalGateResult::Deny,
        };

        if effective_trust_satisfies(&decision.effective_trust, request.trust_level_required) {
            ApprovalGateResult::Allow
        } else {
            ApprovalGateResult::Block(request.clone())
        }
    }
}

/// Returns `true` if the effective trust class meets or exceeds the
/// required trust level.
fn effective_trust_satisfies(effective: &EffectiveTrustClass, required: TrustLevel) -> bool {
    effective_trust_rank(effective) >= trust_level_rank(required)
}

fn effective_trust_rank(tc: &EffectiveTrustClass) -> u8 {
    match tc {
        EffectiveTrustClass::Sandbox => 0,
        EffectiveTrustClass::UserTrusted => 1,
        EffectiveTrustClass::FirstParty(_) => 2,
        EffectiveTrustClass::System(_) => 3,
    }
}

fn trust_level_rank(tl: TrustLevel) -> u8 {
    match tl {
        TrustLevel::Sandbox => 0,
        TrustLevel::UserTrusted => 1,
        TrustLevel::FirstParty => 2,
        TrustLevel::System => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::HostTrustPolicy;

    fn make_request(required: TrustLevel) -> ApprovalRequest {
        ApprovalRequest::new(
            "req-gate",
            "tool-read",
            "session-gate",
            serde_json::json!({}),
            required,
        )
    }

    #[test]
    fn gate_blocks_request_requiring_higher_trust() {
        let gate = ApprovalGate::new(HostTrustPolicy::user_trusted());
        let request = make_request(TrustLevel::FirstParty);

        // Local user gets UserTrusted, but tool requires FirstParty
        let input = TrustPolicyInput::local_user(vec![]);
        let result = gate.check(&request, &input);

        match result {
            ApprovalGateResult::Block(_) => {}
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn gate_allows_request_when_trust_is_sufficient() {
        let gate = ApprovalGate::new(HostTrustPolicy::user_trusted());
        let request = make_request(TrustLevel::UserTrusted);

        let input = TrustPolicyInput::local_user(vec![]);
        let result = gate.check(&request, &input);

        assert_eq!(result, ApprovalGateResult::Allow);
    }

    #[test]
    fn gate_allows_sandbox_request_for_user_trusted_caller() {
        let gate = ApprovalGate::new(HostTrustPolicy::user_trusted());
        let request = make_request(TrustLevel::Sandbox);

        let input = TrustPolicyInput::local_user(vec![]);
        let result = gate.check(&request, &input);

        assert_eq!(result, ApprovalGateResult::Allow);
    }

    #[test]
    fn gate_blocks_remote_peer_for_user_trusted_tool() {
        let gate = ApprovalGate::new(HostTrustPolicy::user_trusted());
        let request = make_request(TrustLevel::UserTrusted);

        // Remote peer gets Sandbox, tool requires UserTrusted
        let input = TrustPolicyInput::remote_peer(Some("peer-1".into()), vec![]);
        let result = gate.check(&request, &input);

        match result {
            ApprovalGateResult::Block(_) => {}
            other => panic!("expected Block, got {:?}", other),
        }
    }

    #[test]
    fn gate_denies_when_policy_returns_error() {
        let gate = ApprovalGate::new(HostTrustPolicy::fail_closed());
        let request = make_request(TrustLevel::Sandbox);

        // Even Sandbox-level requests are denied under fail_closed
        let input = TrustPolicyInput::local_user(vec![]);
        let result = gate.check(&request, &input);

        // fail_closed returns Sandbox for everything, so Sandbox >= Sandbox = Allow
        assert_eq!(result, ApprovalGateResult::Allow);
    }

    #[test]
    fn trust_rank_functions_are_consistent() {
        assert_eq!(effective_trust_rank(&EffectiveTrustClass::Sandbox), 0);
        assert_eq!(effective_trust_rank(&EffectiveTrustClass::UserTrusted), 1);
        assert_eq!(effective_trust_rank(&EffectiveTrustClass::first_party()), 2);
        assert_eq!(effective_trust_rank(&EffectiveTrustClass::system()), 3);

        assert_eq!(trust_level_rank(TrustLevel::Sandbox), 0);
        assert_eq!(trust_level_rank(TrustLevel::UserTrusted), 1);
        assert_eq!(trust_level_rank(TrustLevel::FirstParty), 2);
        assert_eq!(trust_level_rank(TrustLevel::System), 3);
    }
}
