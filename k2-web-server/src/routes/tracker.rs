use std::sync::Arc;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use crate::state::{AppState, TopicPeerEntry, WsEvent};

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
    /// Sub-category (e.g. "Short Clips") — None → tracked as "unknown"
    pub subtopic: Option<String>,
    /// Intent: "buy" | "sell" | "exchange"
    pub action: Option<String>,
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
    {
        let mut tracker = state.tracker_store.write().await;
        let entries = tracker.entry(body.topic.clone()).or_default();

        // Deduplicate: xóa entry cũ của cùng node
        entries.retain(|e| e.node_id != body.node_id);
        entries.push(TopicPeerEntry {
            node_id: body.node_id.clone(),
            endpoint_addr,
            announced_at: now,
            subtopic: body.subtopic.clone(),
            action: body.action.clone(),
        });

        // Giữ tối đa MAX_PEERS_PER_TOPIC entries mới nhất
        if entries.len() > MAX_PEERS_PER_TOPIC {
            entries.sort_by(|a, b| b.announced_at.cmp(&a.announced_at));
            entries.truncate(MAX_PEERS_PER_TOPIC);
        }
    }

    // Broadcast updated stats qua WebSocket
    let stats = compute_subtopic_stats(&state, &body.topic).await;
    let _ = state.event_tx.send(WsEvent::SubtopicStatsUpdated {
        topic: body.topic.clone(),
        stats,
    });

    Ok(Json(json!({ "status": "announced", "topic": body.topic, "node_id": body.node_id })))
}

#[derive(Deserialize)]
pub struct PeersQuery {
    pub topic: String,
}

/// DELETE /api/tracker/announce — xóa node khỏi tracker khi deal xong
pub async fn leave_topic(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AnnounceBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if body.topic.is_empty() || body.node_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "topic and node_id required".to_string()));
    }
    {
        let mut tracker = state.tracker_store.write().await;
        if let Some(entries) = tracker.get_mut(&body.topic) {
            entries.retain(|e| e.node_id != body.node_id);
        }
    }
    let stats = compute_subtopic_stats(&state, &body.topic).await;
    let _ = state.event_tx.send(WsEvent::SubtopicStatsUpdated {
        topic: body.topic.clone(),
        stats,
    });
    Ok(Json(json!({ "status": "left", "topic": body.topic, "node_id": body.node_id })))
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

#[derive(Deserialize)]
pub struct SubtopicStatsQuery {
    pub topic: String,
}

/// GET /api/tracker/subtopic-stats?topic=X
/// Trả về thống kê nodes theo subtopic + action
pub async fn get_subtopic_stats(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SubtopicStatsQuery>,
) -> Json<serde_json::Value> {
    let stats = compute_subtopic_stats(&state, &query.topic).await;
    Json(json!({ "topic": query.topic, "stats": stats }))
}

/// Public alias để các module khác gọi (e.g. marketplace.rs)
pub async fn compute_subtopic_stats_pub(state: &Arc<AppState>, topic: &str) -> serde_json::Value {
    compute_subtopic_stats(state, topic).await
}

/// Aggregates tracker_store + offer_store by subtopic + action cho một topic
async fn compute_subtopic_stats(state: &Arc<AppState>, topic: &str) -> serde_json::Value {
    let now = now_unix();

    // subtopic → action → Vec<node_id>
    let mut map: std::collections::HashMap<
        String,
        std::collections::HashMap<String, Vec<String>>,
    > = std::collections::HashMap::new();

    // 1. Lấy từ tracker_store (live announce)
    {
        let tracker = state.tracker_store.read().await;
        if let Some(entries) = tracker.get(topic) {
            for e in entries.iter().filter(|e| now.saturating_sub(e.announced_at) < PEER_TTL_SECS) {
                let sub = e.subtopic.clone().unwrap_or_else(|| "unknown".to_string());
                let act = e.action.clone().unwrap_or_else(|| "unknown".to_string());
                map.entry(sub).or_default()
                    .entry(act).or_default()
                    .push(e.node_id.clone());
            }
        }
    }

    // 2. Lấy từ offer_store (offers đã post)
    // form_data có cấu trúc: { selection: { subtopic: "..." }, action: "buy", ... }
    {
        let offers = state.offer_store.read().await;
        for offer in offers.iter().filter(|o| {
            o.topic == topic && now.saturating_sub(o.timestamp) < 3600
        }) {
            // Thử lấy từ selection.subtopic trước (cấu trúc DynamicForm)
            let sub = offer.form_data
                .get("selection").and_then(|s| s.get("subtopic")).and_then(|v| v.as_str())
                // Fallback: thử trực tiếp form_data.subtopic
                .or_else(|| offer.form_data.get("subtopic").and_then(|v| v.as_str()))
                // Fallback: thử selection.category (Freelance Job)
                .or_else(|| offer.form_data.get("selection").and_then(|s| s.get("category")).and_then(|v| v.as_str()))
                .unwrap_or("unknown")
                .to_string();
            map.entry(sub).or_default()
                .entry(offer.action.clone()).or_default()
                .push(offer.session_id.clone());
        }
    }

    // 3. Flatten thành JSON array
    let mut result: Vec<serde_json::Value> = map.into_iter().map(|(sub, actions)| {
        let buy      = actions.get("buy").map(|v| v.len()).unwrap_or(0);
        let sell     = actions.get("sell").map(|v| v.len()).unwrap_or(0);
        let exchange = actions.get("exchange").map(|v| v.len()).unwrap_or(0);
        let unknown  = actions.get("unknown").map(|v| v.len()).unwrap_or(0);
        let total    = buy + sell + exchange + unknown;

        let buy_nodes      = actions.get("buy").cloned().unwrap_or_default();
        let sell_nodes     = actions.get("sell").cloned().unwrap_or_default();
        let exchange_nodes = actions.get("exchange").cloned().unwrap_or_default();
        let unknown_nodes  = actions.get("unknown").cloned().unwrap_or_default();

        json!({
            "subtopic":  sub,
            "buy":       buy,
            "sell":      sell,
            "exchange":  exchange,
            "unknown":   unknown,
            "total":     total,
            "nodes": {
                "buy":      buy_nodes,
                "sell":     sell_nodes,
                "exchange": exchange_nodes,
                "unknown":  unknown_nodes,
            }
        })
    }).collect();

    // Sort by total desc
    result.sort_by(|a, b| {
        let ta = a.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        let tb = b.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
        tb.cmp(&ta)
    });

    json!(result)
}
