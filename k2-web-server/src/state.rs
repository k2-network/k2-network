use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{RwLock, broadcast, mpsc};
use serde::{Serialize, Deserialize};
use k2_core::{K2Node, ContactBookDocs};
use sqlx::PgPool;


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
    FriendRequest {
        recipient_session_id: String,
        payload: serde_json::Value,
    },
    FriendRequestResponse {
        recipient_session_id: String,
        payload: serde_json::Value,
    },
    OfferReceived { payload: serde_json::Value },
    OfferMatched { payload: serde_json::Value },
    PeerConnected { node_id: String },
    PeerDisconnected { node_id: String },
    SubtopicStatsUpdated { topic: String, stats: serde_json::Value },
}

/// Một entry trong topic tracker — node đang online trong topic
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TopicPeerEntry {
    pub node_id: String,              // hex public key
    pub endpoint_addr: serde_json::Value, // serialized EndpointAddr (id + addrs)
    pub announced_at: u64,            // unix timestamp
    /// Sub-category trong topic (e.g. "Short Clips"), None → "unknown"
    #[serde(default)]
    pub subtopic: Option<String>,
    /// Intent: "buy" | "sell" | "exchange" | None
    #[serde(default)]
    pub action: Option<String>,
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
    /// Map iroh node_id → ws session_id (UUID) để route chat messages
    pub node_to_session: Arc<RwLock<HashMap<String, String>>>,
    /// PostgreSQL pool
    pub db: PgPool,
    /// JWT secret
    pub jwt_secret: String,
}

impl AppState {
    pub async fn new() -> (Arc<Self>, broadcast::Receiver<WsEvent>) {
        let (event_tx, event_rx) = broadcast::channel(256);
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("[K2] JWT_SECRET env var is required — set a strong random secret (e.g. openssl rand -hex 32)");

        // Init PostgreSQL
        let db_url = std::env::var("DATABASE_URL")
            .expect("[K2] DATABASE_URL env var is required");
        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(10)
            .connect(&db_url)
            .await
            .expect("Failed to connect to PostgreSQL");

        // Create tables if not exists
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS users (
                id TEXT PRIMARY KEY,
                username TEXT NOT NULL UNIQUE,
                email TEXT NOT NULL UNIQUE,
                password_hash TEXT NOT NULL,
                node_id TEXT NOT NULL UNIQUE,
                created_at BIGINT NOT NULL
            )"
        ).execute(&db).await.expect("Failed to create users table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS refresh_tokens (
                jti TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                expires_at BIGINT NOT NULL
            )"
        ).execute(&db).await.expect("Failed to create refresh_tokens table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"
        ).execute(&db).await.expect("Failed to create settings table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS contacts (
                user_id TEXT NOT NULL,
                node_id TEXT NOT NULL,
                nickname TEXT NOT NULL,
                notes TEXT,
                added_at BIGINT NOT NULL,
                PRIMARY KEY (user_id, node_id)
            )"
        ).execute(&db).await.expect("Failed to create contacts table");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS chat_messages (
                id BIGSERIAL PRIMARY KEY,
                session_id TEXT NOT NULL,
                conversation_id TEXT NOT NULL DEFAULT 'ai',
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                sender_name TEXT,
                created_at BIGINT NOT NULL
            )"
        ).execute(&db).await.expect("Failed to create chat_messages table");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_chat_messages_session
             ON chat_messages (session_id, conversation_id, created_at)"
        ).execute(&db).await.expect("Failed to create chat_messages index");

        // Add reply columns if not exist (safe migration)
        let _ = sqlx::query("ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS reply_to_content TEXT")
            .execute(&db).await;
        let _ = sqlx::query("ALTER TABLE chat_messages ADD COLUMN IF NOT EXISTS reply_to_sender TEXT")
            .execute(&db).await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS friend_requests (
                id BIGSERIAL PRIMARY KEY,
                from_user_id TEXT NOT NULL,
                from_node_id TEXT NOT NULL,
                from_username TEXT NOT NULL,
                to_node_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                created_at BIGINT NOT NULL,
                UNIQUE(from_node_id, to_node_id)
            )"
        ).execute(&db).await.expect("Failed to create friend_requests table");

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_friend_requests_to
             ON friend_requests (to_node_id, status)"
        ).execute(&db).await.expect("Failed to create friend_requests index");

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS offers (
                offer_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                topic TEXT NOT NULL,
                action TEXT NOT NULL,
                form_data JSONB NOT NULL,
                timestamp BIGINT NOT NULL
            )"
        ).execute(&db).await.expect("Failed to create offers table");

        // Load active offers từ DB vào memory khi khởi động
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let rows = sqlx::query_as::<_, (String, String, String, String, serde_json::Value, i64)>(
            "SELECT offer_id, session_id, topic, action, form_data, timestamp FROM offers WHERE timestamp > $1"
        )
        .bind(now - 300)
        .fetch_all(&db)
        .await
        .expect("Failed to load offers from DB");

        let loaded_offers: Vec<Offer> = rows.into_iter().map(|(offer_id, session_id, topic, action, form_data, timestamp)| Offer {
            offer_id,
            session_id,
            topic,
            action,
            form_data,
            timestamp: timestamp as u64,
        }).collect();

        println!("[K2] Loaded {} active offers from DB", loaded_offers.len());

        let state = Arc::new(AppState {
            node: Mutex::new(None),
            contacts: Arc::new(RwLock::new(None)),
            topic_senders: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            offer_store: Arc::new(RwLock::new(loaded_offers)),
            tracker_store: Arc::new(RwLock::new(HashMap::new())),
            node_to_session: Arc::new(RwLock::new(HashMap::new())),
            db,
            jwt_secret,
        });
        (state, event_rx)
    }
}
