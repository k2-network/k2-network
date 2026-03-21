use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, broadcast, mpsc};
use serde::{Serialize, Deserialize};
use k2_core::{K2Node, ContactBookDocs};

/// Events emitted by background tasks → sent to all WebSocket clients
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsEvent {
    /// Chat message routed to a specific session
    ChatMessage {
        /// session_id of the intended recipient — WS handler filters by this
        recipient_session_id: String,
        payload: serde_json::Value,
    },
    OfferReceived { payload: serde_json::Value },
    OfferMatched { payload: serde_json::Value },
    PeerConnected { node_id: String },
    PeerDisconnected { node_id: String },
}

/// Một entry trong topic tracker — node đang online trong topic
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicPeerEntry {
    pub node_id: String,              // hex public key
    pub endpoint_addr: serde_json::Value, // serialized EndpointAddr (id + addrs)
    pub announced_at: u64,            // unix timestamp
}

/// Một offer được lưu trong server memory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Offer {
    pub offer_id: String,
    pub session_id: String,
    pub topic: String,
    pub action: String,   // "buy" | "sell" | "exchange"
    pub form_data: serde_json::Value,
    pub timestamp: u64,
}

/// Shared application state
pub struct AppState {
    pub node: Mutex<Option<K2Node>>,
    pub contacts: Arc<RwLock<Option<ContactBookDocs>>>,
    /// Topic senders — for P2P broadcasting (Tauri/multi-server)
    pub topic_senders: Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Vec<u8>>>>>,
    /// Broadcast channel: background tasks push WsEvent; WS clients subscribe
    pub event_tx: broadcast::Sender<WsEvent>,
    /// Web matching engine: lưu offers trong memory, tự match buy↔sell
    pub offer_store: Arc<RwLock<Vec<Offer>>>,
    /// Topic tracker: map topic_name → [TopicPeerEntry] để bootstrap gossip peers
    pub tracker_store: Arc<RwLock<HashMap<String, Vec<TopicPeerEntry>>>>,
}

impl AppState {
    pub fn new() -> (Arc<Self>, broadcast::Receiver<WsEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let state = Arc::new(AppState {
            node: Mutex::new(None),
            contacts: Arc::new(RwLock::new(None)),
            topic_senders: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            offer_store: Arc::new(RwLock::new(Vec::new())),
            tracker_store: Arc::new(RwLock::new(HashMap::new())),
        });
        (state, event_rx)
    }
}
