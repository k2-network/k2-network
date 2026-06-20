//! P2P Inbound Security - handles inbound triggers with trust policies.
//!
//! This module provides security controls for P2P inbound operations:
//! - Validates inbound requests before processing
//! - Enforces trust level requirements for remote peers
//! - Blocks unauthorized inbound capabilities

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::capabilities::context::TrustLevel;
use crate::capabilities::ToolRegistry;

/// Error types for P2P security operations.
#[derive(Debug, thiserror::Error)]
pub enum P2PSecurityError {
    #[error("Insufficient trust: required={required}, actual={actual}")]
    InsufficientTrust {
        required: String,
        actual: String,
    },

    #[error("Unauthorized peer: {peer_id}")]
    UnauthorizedPeer { peer_id: String },

    #[error("Security policy violation: {0}")]
    PolicyViolation(String),

    #[error("Internal security error: {0}")]
    Internal(String),
}

/// Result type for P2P security operations.
pub type P2PSecurityResult<T> = Result<T, P2PSecurityError>;

/// Trust level assignment for remote peers.
#[derive(Debug, Clone)]
pub struct RemotePeerTrust {
    /// Minimum trust level for all remote peers (default: Sandbox).
    pub baseline: TrustLevel,
    
    /// Trusted peers with elevated access.
    pub trusted_peers: Vec<String>,
}

impl Default for RemotePeerTrust {
    fn default() -> Self {
        Self {
            baseline: TrustLevel::Sandbox,
            trusted_peers: Vec::new(),
        }
    }
}

/// P2P Inbound Security - enforces trust policies on inbound requests.
///
/// This component validates inbound P2P requests against trust policies
/// before allowing them to execute. Remote untrusted peers are limited to
/// Sandbox-level tools by default.
#[derive(Clone)]
pub struct P2PInboundSecurity {
    /// Tool registry for checking tool trust requirements.
    tool_registry: Arc<ToolRegistry>,

    /// Remote peer trust settings.
    peer_trust: Arc<RwLock<RemotePeerTrust>>,
    
    /// Blocked peer list (node IDs).
    blocked_peers: Arc<RwLock<Vec<String>>>,
}

impl P2PInboundSecurity {
    /// Create a new P2P inbound security instance.
    ///
    /// # Arguments
    ///
    /// * `tool_registry` - Tool registry for checking tool requirements
    /// * `peer_trust` - Remote peer trust settings
    pub fn new(tool_registry: Arc<ToolRegistry>, peer_trust: RemotePeerTrust) -> Self {
        Self {
            tool_registry,
            peer_trust: Arc::new(RwLock::new(peer_trust)),
            blocked_peers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default settings (sandbox for all remote peers).
    pub fn with_defaults(tool_registry: Arc<ToolRegistry>) -> Self {
        Self::new(tool_registry, RemotePeerTrust::default())
    }

    /// Get the effective trust level for a remote peer.
    async fn get_peer_trust_level(&self, peer_id: &str) -> TrustLevel {
        let trust = self.peer_trust.read().await;
        if trust.trusted_peers.contains(&peer_id.to_string()) {
            TrustLevel::UserTrusted
        } else {
            trust.baseline
        }
    }

    /// Validate an inbound tool invocation request from a remote peer.
    ///
    /// This checks:
    /// 1. Peer is not blocked
    /// 2. Peer trust level meets tool requirements
    ///
    /// # Arguments
    ///
    /// * `peer_id` - Remote peer's node ID (public key as string)
    /// * `tool_id` - Tool being invoked
    /// * `session_id` - Session identifier for tracking
    ///
    /// # Returns
    ///
    /// Ok(()) if the request is allowed, Err otherwise.
    pub async fn validate_inbound_tool_call(
        &self,
        peer_id: &str,
        tool_id: &str,
        _session_id: &str,
    ) -> P2PSecurityResult<()> {
        // Check if peer is blocked
        let blocked = self.blocked_peers.read().await;
        if blocked.contains(&peer_id.to_string()) {
            return Err(P2PSecurityError::UnauthorizedPeer {
                peer_id: peer_id.to_string(),
            });
        }
        drop(blocked);

        // Get tool schema to check trust requirements
        use crate::capabilities::tool::ToolId;
        let tool_id_wrapper: ToolId = tool_id.to_string().into();
        
        let tool_schema = self
            .tool_registry
            .get_schema(&tool_id_wrapper)
            .map_err(|e| P2PSecurityError::PolicyViolation(format!("Tool lookup failed: {}", e)))?;

        let required_trust = tool_schema.required_trust_level;

        // Get peer's trust level
        let peer_trust = self.get_peer_trust_level(peer_id).await;

        // Check if peer has sufficient trust
        if !peer_trust.satisfies(required_trust) {
            return Err(P2PSecurityError::InsufficientTrust {
                required: format!("{:?}", required_trust),
                actual: format!("{:?}", peer_trust),
            });
        }

        Ok(())
    }

    /// Validate an inbound file transfer request.
    ///
    /// File transfers from untrusted peers are blocked by default.
    pub async fn validate_inbound_file_transfer(
        &self,
        peer_id: &str,
        _file_size: u64,
    ) -> P2PSecurityResult<()> {
        // Check if peer is blocked
        let blocked = self.blocked_peers.read().await;
        if blocked.contains(&peer_id.to_string()) {
            return Err(P2PSecurityError::UnauthorizedPeer {
                peer_id: peer_id.to_string(),
            });
        }
        drop(blocked);

        // Require at least UserTrusted level for file transfers
        let peer_trust = self.get_peer_trust_level(peer_id).await;

        if !peer_trust.satisfies(TrustLevel::UserTrusted) {
            return Err(P2PSecurityError::InsufficientTrust {
                required: "UserTrusted".to_string(),
                actual: format!("{:?}", peer_trust),
            });
        }

        Ok(())
    }

    /// Add a peer to the trusted peers list.
    pub async fn trust_peer(&self, peer_id: String) {
        let mut trust = self.peer_trust.write().await;
        if !trust.trusted_peers.contains(&peer_id) {
            trust.trusted_peers.push(peer_id);
        }
    }

    /// Remove a peer from the trusted peers list.
    pub async fn untrust_peer(&self, peer_id: &str) {
        let mut trust = self.peer_trust.write().await;
        trust.trusted_peers.retain(|id| id != peer_id);
    }

    /// Block a peer from making inbound requests.
    pub async fn block_peer(&self, peer_id: String) {
        let mut blocked = self.blocked_peers.write().await;
        if !blocked.contains(&peer_id) {
            blocked.push(peer_id);
        }
    }

    /// Unblock a previously blocked peer.
    pub async fn unblock_peer(&self, peer_id: &str) {
        let mut blocked = self.blocked_peers.write().await;
        blocked.retain(|id| id != peer_id);
    }

    /// Check if a peer is currently blocked.
    pub async fn is_peer_blocked(&self, peer_id: &str) -> bool {
        let blocked = self.blocked_peers.read().await;
        blocked.contains(&peer_id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::ToolRegistry;
    use crate::tools::register_all;

    fn make_test_security() -> P2PInboundSecurity {
        let mut tool_registry = ToolRegistry::new();
        register_all(&mut tool_registry).unwrap();
        
        P2PInboundSecurity::with_defaults(Arc::new(tool_registry))
    }

    #[tokio::test]
    async fn test_validate_inbound_tool_call_insufficient_trust() {
        let security = make_test_security();
        
        // Remote peer with Sandbox trust should fail for tools requiring UserTrusted
        let result = security.validate_inbound_tool_call(
            "remote-peer-123",
            "file_read",
            "session-1",
        ).await;
        
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, P2PSecurityError::InsufficientTrust { .. }));
    }

    #[tokio::test]
    async fn test_trusted_peer_can_access_user_trusted_tools() {
        let security = make_test_security();
        let peer_id = "trusted-peer-456";
        
        // Trust the peer
        security.trust_peer(peer_id.to_string()).await;
        
        // Now access should succeed
        let result = security.validate_inbound_tool_call(
            peer_id,
            "file_read",
            "session-1",
        ).await;
        
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_block_and_unblock_peer() {
        let security = make_test_security();
        let peer_id = "bad-peer-123";
        
        // Initially not blocked
        assert!(!security.is_peer_blocked(peer_id).await);
        
        // Block the peer
        security.block_peer(peer_id.to_string()).await;
        assert!(security.is_peer_blocked(peer_id).await);
        
        // Requests from blocked peer should fail
        let result = security.validate_inbound_tool_call(peer_id, "http_get", "session-1").await;
        assert!(matches!(result, Err(P2PSecurityError::UnauthorizedPeer { .. })));
        
        // Unblock the peer
        security.unblock_peer(peer_id).await;
        assert!(!security.is_peer_blocked(peer_id).await);
    }

    #[tokio::test]
    async fn test_validate_inbound_file_transfer_insufficient_trust() {
        let security = make_test_security();
        
        // Remote peer with Sandbox trust should fail for file transfer
        let result = security.validate_inbound_file_transfer("remote-peer-456", 1024).await;
        assert!(result.is_err());
    }
}