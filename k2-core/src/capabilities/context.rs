//! Execution context and trust model for the capabilities module.
//!
//! Every tool invocation carries an `ExecutionContext` that describes who is
//! calling the tool and under what constraints. The `TrustLevel` enum gates
//! which tools may be invoked by which callers.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// The trust level of an execution context.
///
/// Higher trust levels grant access to more privileged tools.
/// The ordering is: `Sandbox < UserTrusted < FirstParty < System`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Lowest trust — tools running in a sandboxed environment (WASM).
    Sandbox,
    /// Tools invoked by a user who has explicitly granted trust.
    UserTrusted,
    /// Tools authored by the same party that operates the node.
    FirstParty,
    /// Highest trust — system-level tools with full node access.
    System,
}

impl TrustLevel {
    /// Returns `true` if `self` is at least as trusted as `other`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// assert!(TrustLevel::System.satisfies(TrustLevel::FirstParty));
    /// assert!(TrustLevel::FirstParty.satisfies(TrustLevel::UserTrusted));
    /// assert!(!TrustLevel::Sandbox.satisfies(TrustLevel::UserTrusted));
    /// ```
    pub fn satisfies(self, required: TrustLevel) -> bool {
        self >= required
    }
}

impl PartialOrd for TrustLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TrustLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        self.numeric().cmp(&other.numeric())
    }
}

impl TrustLevel {
    /// Return a numeric representation for ordering comparisons.
    fn numeric(self) -> u8 {
        match self {
            TrustLevel::Sandbox => 0,
            TrustLevel::UserTrusted => 1,
            TrustLevel::FirstParty => 2,
            TrustLevel::System => 3,
        }
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrustLevel::Sandbox => write!(f, "sandbox"),
            TrustLevel::UserTrusted => write!(f, "user_trusted"),
            TrustLevel::FirstParty => write!(f, "first_party"),
            TrustLevel::System => write!(f, "system"),
        }
    }
}

/// Context for a tool invocation — who is calling, from where, at what trust.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// The K2 node that originated this invocation.
    pub node_id: String,
    /// Unique identifier for this session (or tool-use chain).
    pub session_id: String,
    /// The trust level granted to this invocation.
    pub trust_level: TrustLevel,
    /// If the invocation was triggered by a remote peer, their node id.
    pub peer_id: Option<String>,
}

impl ExecutionContext {
    /// Create a new execution context.
    pub fn new(
        node_id: String,
        session_id: String,
        trust_level: TrustLevel,
        peer_id: Option<String>,
    ) -> Self {
        Self {
            node_id,
            session_id,
            trust_level,
            peer_id,
        }
    }

    /// Convenience constructor: system-level context for the local node.
    pub fn system(node_id: impl Into<String>, session_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            session_id: session_id.into(),
            trust_level: TrustLevel::System,
            peer_id: None,
        }
    }

    /// Convenience constructor: sandbox-level context (WASM tools).
    pub fn sandbox(
        node_id: impl Into<String>,
        session_id: impl Into<String>,
        peer_id: Option<String>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            session_id: session_id.into(),
            trust_level: TrustLevel::Sandbox,
            peer_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_level_ordering() {
        assert!(TrustLevel::System > TrustLevel::FirstParty);
        assert!(TrustLevel::FirstParty > TrustLevel::UserTrusted);
        assert!(TrustLevel::UserTrusted > TrustLevel::Sandbox);
        assert!(TrustLevel::Sandbox < TrustLevel::System);
    }

    #[test]
    fn trust_level_satisfies() {
        assert!(TrustLevel::System.satisfies(TrustLevel::Sandbox));
        assert!(TrustLevel::System.satisfies(TrustLevel::FirstParty));
        assert!(TrustLevel::FirstParty.satisfies(TrustLevel::UserTrusted));
        assert!(!TrustLevel::UserTrusted.satisfies(TrustLevel::FirstParty));
        assert!(!TrustLevel::Sandbox.satisfies(TrustLevel::System));
    }

    #[test]
    fn trust_level_serde_roundtrip() {
        let levels = [
            TrustLevel::Sandbox,
            TrustLevel::UserTrusted,
            TrustLevel::FirstParty,
            TrustLevel::System,
        ];
        for level in &levels {
            let json = serde_json::to_string(level).unwrap();
            let round: TrustLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, round);
        }
    }

    #[test]
    fn execution_context_constructors() {
        let ctx = ExecutionContext::system("node-1", "sess-1");
        assert_eq!(ctx.node_id, "node-1");
        assert_eq!(ctx.session_id, "sess-1");
        assert_eq!(ctx.trust_level, TrustLevel::System);
        assert!(ctx.peer_id.is_none());

        let ctx = ExecutionContext::sandbox("node-2", "sess-2", Some("peer-1".into()));
        assert_eq!(ctx.trust_level, TrustLevel::Sandbox);
        assert_eq!(ctx.peer_id, Some("peer-1".into()));
    }
}
