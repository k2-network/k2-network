//! Trust invalidation bus.
//!
//! Provides [`InvalidationBus`] — a `tokio::sync::broadcast`-based
//! publish/subscribe channel for notifying consumers when a trust
//! relationship changes (e.g. a peer is upgraded, downgraded, or
//! has its grant revoked).
//!
//! # Example
//!
//! ```rust,ignore
//! let bus = InvalidationBus::new(16);
//! let mut listener = bus.subscribe();
//!
//! bus.publish(TrustChange::PeerDowngraded {
//!     peer_id: "abc".into(),
//!     new_class: EffectiveTrustClass::sandbox(),
//! });
//!
//! let change = listener.recv().await.unwrap();
//! ```

use tokio::sync::broadcast;

use crate::security::error::{TrustError, TrustResult};

use super::trust::EffectiveTrustClass;

// ---------------------------------------------------------------------------
// TrustChange
// ---------------------------------------------------------------------------

/// A change in trust status broadcast on the invalidation bus.
///
/// Consumers should react to these events by re-evaluating cached
/// trust decisions and, when appropriate, revoking access tokens
/// or aborting in-flight operations.
#[derive(Debug, Clone)]
pub enum TrustChange {
    /// A previously granted permission has been revoked entirely.
    GrantRevoked {
        /// The peer whose grant was revoked.
        peer_id: String,
        /// Optional human-readable reason.
        reason: Option<String>,
    },

    /// The authority ceiling for a principal has been lowered.
    AuthorityCeilingReduced {
        /// The affected peer.
        peer_id: String,
        /// Optional human-readable reason.
        reason: Option<String>,
    },

    /// A peer has been **upgraded** to a higher trust class.
    PeerUpgraded {
        /// The peer that was upgraded.
        peer_id: String,
        /// The new, higher trust class.
        new_class: EffectiveTrustClass,
    },

    /// A peer has been **downgraded** to a lower trust class.
    PeerDowngraded {
        /// The peer that was downgraded.
        peer_id: String,
        /// The new, lower trust class.
        new_class: EffectiveTrustClass,
    },
}

// ---------------------------------------------------------------------------
// TrustChangeListener
// ---------------------------------------------------------------------------

/// A subscriber handle returned by [`InvalidationBus::subscribe`].
///
/// Call [`recv`](TrustChangeListener::recv) to await the next
/// [`TrustChange`] notification.
#[derive(Debug)]
pub struct TrustChangeListener {
    rx: broadcast::Receiver<TrustChange>,
}

impl TrustChangeListener {
    /// Await the next [`TrustChange`] published on the bus.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::InvalidationFailed`] if the channel is closed
    /// (all senders dropped) or if the receiver has lagged too far behind.
    pub async fn recv(&mut self) -> TrustResult<TrustChange> {
        self.rx
            .recv()
            .await
            .map_err(|e| TrustError::InvalidationFailed(e.to_string()))
    }

    /// Non-blocking try-recv. Returns `None` if no message is available.
    pub fn try_recv(&mut self) -> TrustResult<Option<TrustChange>> {
        match self.rx.try_recv() {
            Ok(change) => Ok(Some(change)),
            Err(broadcast::error::TryRecvError::Empty) => Ok(None),
            Err(broadcast::error::TryRecvError::Closed) => {
                Err(TrustError::InvalidationFailed("channel closed".into()))
            }
            Err(broadcast::error::TryRecvError::Lagged(n)) => Err(TrustError::InvalidationFailed(
                format!("receiver lagged by {} messages", n),
            )),
        }
    }

    /// Re-subscribe, resetting any lagged state.
    pub fn resubscribe(&mut self, bus: &InvalidationBus) {
        self.rx = bus.tx.subscribe();
    }
}

// ---------------------------------------------------------------------------
// InvalidationBus
// ---------------------------------------------------------------------------

/// A broadcast channel for trust-change notifications.
///
/// Internally backed by [`tokio::sync::broadcast`].  Multiple listeners
/// can subscribe and each will receive every published [`TrustChange`].
///
/// # Cloning
///
/// `InvalidationBus` is cheap to clone (it clones the internal sender
/// handle).  All clones publish to the same channel.
#[derive(Debug, Clone)]
pub struct InvalidationBus {
    tx: broadcast::Sender<TrustChange>,
}

impl InvalidationBus {
    /// Create a new invalidation bus with the given channel capacity.
    ///
    /// The capacity determines how many messages can be buffered before
    /// slow receivers start lagging.  A typical value is 16 or 32.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe to trust-change notifications.
    ///
    /// The returned [`TrustChangeListener`] will receive all future
    /// [`TrustChange`] messages published on this bus.
    pub fn subscribe(&self) -> TrustChangeListener {
        TrustChangeListener {
            rx: self.tx.subscribe(),
        }
    }

    /// Publish a trust change to all subscribers.
    ///
    /// Returns `Ok(())` if at least one subscriber received the message.
    ///
    /// # Errors
    ///
    /// Returns [`TrustError::NoSubscribers`] if no receivers are
    /// currently attached to the bus.
    pub fn publish(&self, change: TrustChange) -> TrustResult<()> {
        self.tx
            .send(change)
            .map_err(|e| match e {
                broadcast::error::SendError(_) => TrustError::NoSubscribers,
            })?;
        Ok(())
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_publish_subscribe_single() {
        let bus = InvalidationBus::new(8);
        let mut listener = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 1);

        bus.publish(TrustChange::GrantRevoked {
            peer_id: "peer1".into(),
            reason: Some("test".into()),
        })
        .expect("publish should succeed");

        let change = listener.recv().await.expect("should receive message");
        assert!(matches!(change, TrustChange::GrantRevoked { .. }));
    }

    #[tokio::test]
    async fn test_publish_subscribe_multiple_listeners() {
        let bus = InvalidationBus::new(8);
        let mut a = bus.subscribe();
        let mut b = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(TrustChange::PeerUpgraded {
            peer_id: "peer2".into(),
            new_class: EffectiveTrustClass::user_trusted(),
        })
        .expect("publish should succeed");

        let change_a = a.recv().await.expect("a should receive");
        let change_b = b.recv().await.expect("b should receive");

        assert!(matches!(change_a, TrustChange::PeerUpgraded { .. }));
        assert!(matches!(change_b, TrustChange::PeerUpgraded { .. }));
    }

    #[tokio::test]
    async fn test_publish_no_subscribers_is_error() {
        let bus = InvalidationBus::new(8);
        // No subscribers — publish should fail.
        let result = bus.publish(TrustChange::AuthorityCeilingReduced {
            peer_id: "nobody".into(),
            reason: None,
        });
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_try_recv_empty() {
        let bus = InvalidationBus::new(8);
        let mut listener = bus.subscribe();

        let result = listener.try_recv().expect("try_recv should not error on empty");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_listener_resubscribe() {
        let bus = InvalidationBus::new(8);
        let mut listener = bus.subscribe();

        bus.publish(TrustChange::PeerDowngraded {
            peer_id: "peer3".into(),
            new_class: EffectiveTrustClass::sandbox(),
        })
        .expect("publish should succeed");

        // Consume the first message
        let _ = listener.recv().await;

        // Resubscribe and verify we can still receive
        listener.resubscribe(&bus);

        bus.publish(TrustChange::PeerUpgraded {
            peer_id: "peer3".into(),
            new_class: EffectiveTrustClass::user_trusted(),
        })
        .expect("publish should succeed");

        let change = listener.recv().await.expect("should receive after resubscribe");
        assert!(matches!(change, TrustChange::PeerUpgraded { .. }));
    }

    #[test]
    fn test_trust_change_variants_constructible() {
        let _ = TrustChange::GrantRevoked {
            peer_id: "p".into(),
            reason: Some("r".into()),
        };
        let _ = TrustChange::AuthorityCeilingReduced {
            peer_id: "p".into(),
            reason: None,
        };
        let _ = TrustChange::PeerUpgraded {
            peer_id: "p".into(),
            new_class: EffectiveTrustClass::first_party(),
        };
        let _ = TrustChange::PeerDowngraded {
            peer_id: "p".into(),
            new_class: EffectiveTrustClass::sandbox(),
        };
    }
}
