//! Core trust types: [`EffectiveTrustClass`], [`AuthorityCeiling`],
//! [`TrustDecision`], [`TrustProvenance`], and the [`TrustPolicy`] trait.
//!
//! ## Trust Class Hierarchy
//!
//! ```text
//! Sandbox < UserTrusted < FirstParty < System
//! ```
//!
//! Higher trust classes inherit all privileges of lower ones.
//! [`FirstParty`] and [`System`] are crate-private — only the
//! `k2-core` crate can mint them; external consumers may only
//! pattern-match.

use std::fmt;

use uuid::Uuid;

use crate::security::error::TrustResult;

// ---------------------------------------------------------------------------
// Private construction tokens — external crates cannot name these types.
// ---------------------------------------------------------------------------

/// Private token required to construct [`EffectiveTrustClass::FirstParty`].
/// This type is `pub(crate)` so only `k2-core` internal code can mint
/// FirstParty trust.
///
/// Private token required to construct [`EffectiveTrustClass::FirstParty`].
/// This type is `pub(crate)` so only `k2-core` internal code can mint
/// FirstParty trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FirstPartyToken;

/// Private token required to construct [`EffectiveTrustClass::System`].
/// This type is `pub(crate)` so only `k2-core` internal code can mint
/// System trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SystemToken;

// ---------------------------------------------------------------------------
// EffectiveTrustClass
// ---------------------------------------------------------------------------

/// The effective trust level assigned to a principal after policy evaluation.
///
/// # Ordering
///
/// `Sandbox < UserTrusted < FirstParty < System`.  A higher class implies
/// all privileges of lower classes, so the decision engine can use a simple
/// `>=` comparison to gate access.
///
/// # Construction
///
/// - [`Sandbox`] and [`UserTrusted`] are **public** — any caller may
///   construct them.
/// - [`FirstParty`] and [`System`] require a crate-private token
///   ([`FirstPartyToken`] / [`SystemToken`]) and can therefore only be minted
///   by `k2-core` itself.
///
/// The `private_interfaces` lint is intentionally suppressed for
/// `FirstParty` and `System` variants — they must remain visible for
/// pattern-matching in external crates, but the `pub(crate)` token types
/// prevent construction.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(private_interfaces)]
pub enum EffectiveTrustClass {
    /// Least-privileged sandbox.  All effects denied by default.
    Sandbox,

    /// Trusted by a local human user.  May perform user-facing operations.
    UserTrusted,

    /// First-party code shipping with K2.  Requires [`FirstPartyToken`].
    FirstParty(FirstPartyToken),

    /// System-level authority (e.g. core daemon internals).
    /// Requires [`SystemToken`].
    System(SystemToken),
}

impl EffectiveTrustClass {
    // -- Public constructors -------------------------------------------------

    /// Create a [`Sandbox`] trust class — the default, least-privileged
    /// level.
    pub fn sandbox() -> Self {
        Self::Sandbox
    }

    /// Create a [`UserTrusted`] trust class.
    pub fn user_trusted() -> Self {
        Self::UserTrusted
    }

    // -- Crate-private constructors ------------------------------------------

    /// Create a [`FirstParty`] trust class (crate-private).
    #[allow(dead_code)]
    pub(crate) fn first_party() -> Self {
        Self::FirstParty(FirstPartyToken)
    }

    /// Create a [`System`] trust class (crate-private).
    #[allow(dead_code)]
    pub(crate) fn system() -> Self {
        Self::System(SystemToken)
    }

    // -- Helpers -------------------------------------------------------------

    /// Numeric rank for ordering comparisons.
    ///
    /// Returns 0 for Sandbox, 1 for UserTrusted, 2 for FirstParty,
    /// 3 for System.
    fn rank(&self) -> u8 {
        match self {
            Self::Sandbox => 0,
            Self::UserTrusted => 1,
            Self::FirstParty(_) => 2,
            Self::System(_) => 3,
        }
    }

    /// Returns `true` if this class is at least as trusted as `other`.
    pub fn at_least(&self, other: &Self) -> bool {
        self.rank() >= other.rank()
    }

    /// Human-readable label for logging / error messages.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sandbox => "Sandbox",
            Self::UserTrusted => "UserTrusted",
            Self::FirstParty(_) => "FirstParty",
            Self::System(_) => "System",
        }
    }
}

impl fmt::Display for EffectiveTrustClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

// Ordering: declaration order matches the intended hierarchy.
impl PartialOrd for EffectiveTrustClass {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EffectiveTrustClass {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.rank().cmp(&other.rank())
    }
}

// ---------------------------------------------------------------------------
// EffectKind
// ---------------------------------------------------------------------------

/// Kinds of side-effects or capabilities a principal can request.
///
/// Used in [`AuthorityCeiling`] to express fine-grained allow-lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectKind {
    /// Read data from the local store / blob store.
    Read,

    /// Write data to the local store / blob store.
    Write,

    /// Make outbound network requests.
    Network,

    /// Access the local filesystem (outside the sandbox root).
    Filesystem,

    /// Execute external processes or plugins.
    Execute,

    /// Perform administrative actions (e.g. modify trust policy itself).
    Admin,
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Read => "Read",
            Self::Write => "Write",
            Self::Network => "Network",
            Self::Filesystem => "Filesystem",
            Self::Execute => "Execute",
            Self::Admin => "Admin",
        })
    }
}

// ---------------------------------------------------------------------------
// ResourceCeiling
// ---------------------------------------------------------------------------

/// Quantitative resource bounds enforced by the authority ceiling.
///
/// When a principal's authority ceiling includes a [`ResourceCeiling`],
/// the runtime must not exceed these limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceCeiling {
    /// Maximum memory in megabytes the principal may allocate.
    pub max_memory_mb: u32,

    /// Maximum cumulative CPU time in milliseconds.
    pub max_cpu_time_ms: u64,

    /// Maximum number of function / plugin invocations.
    pub max_invocations: u64,
}

impl Default for ResourceCeiling {
    fn default() -> Self {
        Self {
            max_memory_mb: 64,
            max_cpu_time_ms: 30_000, // 30 seconds
            max_invocations: 100,
        }
    }
}

impl ResourceCeiling {
    /// A very restrictive ceiling suitable for the Sandbox class.
    pub fn sandbox() -> Self {
        Self {
            max_memory_mb: 8,
            max_cpu_time_ms: 5_000,
            max_invocations: 10,
        }
    }

    /// A generous ceiling for internal / system use.
    pub fn generous() -> Self {
        Self {
            max_memory_mb: 4096,
            max_cpu_time_ms: 300_000, // 5 minutes
            max_invocations: 10_000,
        }
    }
}

// ---------------------------------------------------------------------------
// AuthorityCeiling
// ---------------------------------------------------------------------------

/// The set of effects a principal is permitted to perform, along with
/// optional quantitative resource bounds.
///
/// An empty `allowed_effects` vector means **deny all**.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorityCeiling {
    /// The effect kinds this principal is permitted to execute.
    pub allowed_effects: Vec<EffectKind>,

    /// Optional resource limits; `None` means no ceiling (unbounded).
    pub max_resource_ceiling: Option<ResourceCeiling>,
}

impl Default for AuthorityCeiling {
    /// Returns a **deny-all** ceiling — no effects allowed, no resources.
    fn default() -> Self {
        Self {
            allowed_effects: Vec::new(),
            max_resource_ceiling: Some(ResourceCeiling::sandbox()),
        }
    }
}

impl AuthorityCeiling {
    /// Returns a fully **permissive** ceiling (all effects, generous
    /// resources).  Should only be used for [`System`] or
    /// [`FirstParty`] trust classes.
    pub fn permissive() -> Self {
        Self {
            allowed_effects: vec![
                EffectKind::Read,
                EffectKind::Write,
                EffectKind::Network,
                EffectKind::Filesystem,
                EffectKind::Execute,
                EffectKind::Admin,
            ],
            max_resource_ceiling: Some(ResourceCeiling::generous()),
        }
    }

    /// Returns a user-facing ceiling (no Admin, moderate resources).
    pub fn user() -> Self {
        Self {
            allowed_effects: vec![
                EffectKind::Read,
                EffectKind::Write,
                EffectKind::Network,
                EffectKind::Filesystem,
                EffectKind::Execute,
            ],
            max_resource_ceiling: Some(ResourceCeiling::default()),
        }
    }

    /// Returns `true` if the given effect is allowed by this ceiling.
    pub fn allows(&self, effect: EffectKind) -> bool {
        self.allowed_effects.contains(&effect)
    }

    /// Returns `true` if **all** of the given effects are allowed.
    pub fn allows_all(&self, effects: &[EffectKind]) -> bool {
        effects.iter().all(|e| self.allows(*e))
    }
}

// ---------------------------------------------------------------------------
// TrustSource
// ---------------------------------------------------------------------------

/// Where a trust evaluation request originated.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TrustSource {
    /// A human user interacting directly with the local node.
    LocalUser,

    /// A peer on the P2P network (identified by their iroh `PublicKey`).
    RemotePeer,

    /// An internal system process or daemon.
    SystemProcess,

    /// A WASM plugin running inside a sandboxed runtime.
    WASMPlugin,
}

impl fmt::Display for TrustSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::LocalUser => "LocalUser",
            Self::RemotePeer => "RemotePeer",
            Self::SystemProcess => "SystemProcess",
            Self::WASMPlugin => "WASMPlugin",
        })
    }
}

// ---------------------------------------------------------------------------
// TrustPolicyInput
// ---------------------------------------------------------------------------

/// Input to [`TrustPolicy::evaluate`].
#[derive(Debug, Clone)]
pub struct TrustPolicyInput {
    /// Optional peer identifier (e.g. iroh `PublicKey` hex string) when
    /// the request comes from a remote peer.
    pub peer_id: Option<String>,

    /// The origin of the trust request.
    pub source: TrustSource,

    /// The set of effects the caller is requesting.
    pub requested_effects: Vec<EffectKind>,
}

impl TrustPolicyInput {
    /// Create input from a local user requesting the given effects.
    pub fn local_user(requested_effects: Vec<EffectKind>) -> Self {
        Self {
            peer_id: None,
            source: TrustSource::LocalUser,
            requested_effects,
        }
    }

    /// Create input from a remote peer with an optional peer ID.
    pub fn remote_peer(
        peer_id: Option<String>,
        requested_effects: Vec<EffectKind>,
    ) -> Self {
        Self {
            peer_id,
            source: TrustSource::RemotePeer,
            requested_effects,
        }
    }
}

// ---------------------------------------------------------------------------
// TrustProvenance
// ---------------------------------------------------------------------------

/// Metadata describing how and why a particular trust decision was reached.
#[derive(Debug, Clone)]
pub struct TrustProvenance {
    /// Unique identifier for this trust decision.
    pub id: Uuid,

    /// The origin of the trust request that triggered evaluation.
    pub source: TrustSource,

    /// Human-readable description of the reasoning behind the decision.
    pub reason: String,
}

impl TrustProvenance {
    /// Create a new provenance record.
    pub fn new(source: TrustSource, reason: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            source,
            reason: reason.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// TrustDecision
// ---------------------------------------------------------------------------

/// The result of a [`TrustPolicy::evaluate`] call.
///
/// Bundles the assigned effective trust class, the authority ceiling
/// describing what the principal is allowed to do, and provenance metadata.
#[derive(Debug, Clone)]
pub struct TrustDecision {
    /// The effective trust class assigned by the policy.
    pub effective_trust: EffectiveTrustClass,

    /// The authority ceiling — which effects and resource limits apply.
    pub authority_ceiling: AuthorityCeiling,

    /// Metadata about how this decision was reached.
    pub provenance: TrustProvenance,
}

impl TrustDecision {
    /// Convenience: create a deny-everything Sandbox decision.
    pub fn sandbox(reason: impl Into<String>) -> Self {
        Self {
            effective_trust: EffectiveTrustClass::Sandbox,
            authority_ceiling: AuthorityCeiling::default(),
            provenance: TrustProvenance::new(TrustSource::LocalUser, reason),
        }
    }
}

// ---------------------------------------------------------------------------
// TrustPolicy trait
// ---------------------------------------------------------------------------

/// Trait for types that can evaluate trust decisions.
///
/// Implementors examine the [`TrustPolicyInput`] and return either a
/// [`TrustDecision`] granting certain privileges or a [`TrustError`]
/// denying the request outright.
pub trait TrustPolicy {
    /// Evaluate the given trust input and produce a decision.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::PolicyDenied`] if the request cannot be
    /// authorised under any circumstances.  Implementors may also return
    /// other [`TrustError`] variants for internal failures.
    fn evaluate(&self, input: &TrustPolicyInput) -> TrustResult<TrustDecision>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- EffectiveTrustClass ordering ---------------------------------------

    #[test]
    fn effective_trust_class_ordering() {
        assert!(EffectiveTrustClass::Sandbox < EffectiveTrustClass::UserTrusted);
        assert!(EffectiveTrustClass::UserTrusted < EffectiveTrustClass::first_party());
        assert!(EffectiveTrustClass::first_party() < EffectiveTrustClass::system());

        // at_least helper
        assert!(EffectiveTrustClass::system().at_least(&EffectiveTrustClass::Sandbox));
        assert!(EffectiveTrustClass::first_party().at_least(&EffectiveTrustClass::UserTrusted));
        assert!(!EffectiveTrustClass::Sandbox.at_least(&EffectiveTrustClass::UserTrusted));
    }

    #[test]
    fn sandbox_and_user_trusted_constructible_publicly() {
        let s = EffectiveTrustClass::sandbox();
        let u = EffectiveTrustClass::user_trusted();
        assert_eq!(s, EffectiveTrustClass::Sandbox);
        assert_eq!(u, EffectiveTrustClass::UserTrusted);
    }

    #[test]
    fn first_party_and_system_constructible_in_crate() {
        // These constructors are pub(crate), so they work inside k2-core.
        let fp = EffectiveTrustClass::first_party();
        let sys = EffectiveTrustClass::system();
        assert!(matches!(fp, EffectiveTrustClass::FirstParty(_)));
        assert!(matches!(sys, EffectiveTrustClass::System(_)));
    }

    // -- AuthorityCeiling ---------------------------------------------------

    #[test]
    fn default_ceiling_denies_all() {
        let ceiling = AuthorityCeiling::default();
        assert!(!ceiling.allows(EffectKind::Read));
        assert!(!ceiling.allows(EffectKind::Admin));
    }

    #[test]
    fn permissive_ceiling_allows_all() {
        let ceiling = AuthorityCeiling::permissive();
        assert!(ceiling.allows(EffectKind::Read));
        assert!(ceiling.allows(EffectKind::Admin));
        assert!(ceiling.allows_all(&[
            EffectKind::Read,
            EffectKind::Write,
            EffectKind::Network,
        ]));
    }

    #[test]
    fn user_ceiling_allows_no_admin() {
        let ceiling = AuthorityCeiling::user();
        assert!(ceiling.allows(EffectKind::Read));
        assert!(ceiling.allows(EffectKind::Execute));
        assert!(!ceiling.allows(EffectKind::Admin));
    }

    // -- TrustDecision ------------------------------------------------------

    #[test]
    fn sandbox_decision_denies_everything() {
        let decision = TrustDecision::sandbox("test");
        assert_eq!(decision.effective_trust, EffectiveTrustClass::Sandbox);
        assert!(decision.authority_ceiling.allowed_effects.is_empty());
    }

    // -- TrustPolicyInput ---------------------------------------------------

    #[test]
    fn trust_policy_input_local_user() {
        let input = TrustPolicyInput::local_user(vec![EffectKind::Read]);
        assert!(matches!(input.source, TrustSource::LocalUser));
        assert!(input.peer_id.is_none());
    }

    #[test]
    fn trust_policy_input_remote_peer() {
        let input = TrustPolicyInput::remote_peer(
            Some("abc123".into()),
            vec![EffectKind::Network],
        );
        assert!(matches!(input.source, TrustSource::RemotePeer));
        assert_eq!(input.peer_id.as_deref(), Some("abc123"));
    }
}
