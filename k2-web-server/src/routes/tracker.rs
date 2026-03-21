use std::sync::Arc;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use crate::state::{AppState, TopicPeerEntry};

const PEER_TTL_SECS: u64 = 3600; // entries expire sau 1 giờ
const MAX_PEERS_PER_TOPIC: usize = 20;

pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Deserialize)]
pub struct AnnounceBody {
    pub topic: String,
    pub node_id: String,
    /// Serialized EndpointAddr JSON — chứa relay URL và direct addresses
    pub endpoint_addr: Option<serde_json::Value>,
}

/// POST /api/tracker/announce — node báo mình đang online trong topic
pub async fn announce_topic(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnnounceBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if body.topic.is_empty() || body.node_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "topic and node_id required".to_string()));
    }

    // Validate node_id là 64-char hex
    if body.node_id.len() != 64 || hex::decode(&body.node_id).is_err() {
        return Err((StatusCode::BAD_REQUEST, "node_id must be 64-char hex".to_string()));
    }

    let endpoint_addr = body.endpoint_addr.unwrap_or(serde_json::Value::Null);

    let now = now_unix();
    let mut tracker = state.tracker_store.write().await;
    let entries = tracker.entry(body.topic.clone()).or_default();

    // Deduplicate: xóa entry cũ của cùng node
    entries.retain(|e| e.node_id != body.node_id);
    entries.push(TopicPeerEntry {
        node_id: body.node_id.clone(),
        endpoint_addr,
        announced_at: now,
    });

    // Giữ tối đa MAX_PEERS_PER_TOPIC entries mới nhất
    if entries.len() > MAX_PEERS_PER_TOPIC {
        entries.sort_by(|a, b| b.announced_at.cmp(&a.announced_at));
        entries.truncate(MAX_PEERS_PER_TOPIC);
    }

    Ok(Json(json!({ "status": "announced", "topic": body.topic, "node_id": body.node_id })))
}

#[derive(Deserialize)]
pub struct PeersQuery {
    pub topic: String,
}

/// GET /api/tracker/peers?topic=X — lấy danh sách peers đang online trong topic
pub async fn get_topic_peers(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PeersQuery>,
) -> Json<serde_json::Value> {
    let now = now_unix();
    let tracker = state.tracker_store.read().await;

    let peers: Vec<&TopicPeerEntry> = tracker
        .get(&query.topic)
        .map(|entries| {
            entries
                .iter()
                .filter(|e| now.saturating_sub(e.announced_at) < PEER_TTL_SECS)
                .collect()
        })
        .unwrap_or_default();

    Json(json!({ "topic": query.topic, "peers": peers, "count": peers.len() }))
}
