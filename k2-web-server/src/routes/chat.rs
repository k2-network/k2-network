use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use crate::state::{AppState, WsEvent};

#[derive(Deserialize)]
pub struct SendChatBody {
    /// session_id (node_id) of the sender — used to route reply to sender's WS
    pub sender_session_id: String,
    /// session_id (node_id) of the recipient
    pub recipient_node_id: String,
    pub sender_name: String,
    pub content: String,
}

/// POST /api/chat/send
/// Server relay: receives message from sender, forwards via WS broadcast to recipient.
/// No gossip/P2P involved — purely server-side routing.
pub async fn send_chat_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendChatBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let payload = json!({
        "sender_node_id": body.sender_session_id,
        "sender_name": body.sender_name,
        "content": body.content,
        "timestamp": timestamp,
    });

    // Emit to recipient's WebSocket
    let _ = state.event_tx.send(WsEvent::ChatMessage {
        recipient_session_id: body.recipient_node_id.clone(),
        payload,
    });

    Ok(Json(json!({ "status": "sent" })))
}

#[derive(Deserialize)]
pub struct SendP2pBody {
    pub recipient_node_id: String,
    pub sender_session_id: String,
    pub sender_name: String,
    pub content: String,
}

/// POST /api/chat/send-p2p
/// Sends a direct P2P message via Iroh to the recipient's K2Node.
pub async fn send_p2p_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendP2pBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let msg = json!({
        "sender_node_id": body.sender_session_id,
        "sender_name": body.sender_name,
        "content": body.content,
        "timestamp": timestamp,
    });
    let msg_bytes = serde_json::to_vec(&msg)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Clone node out of std::sync::Mutex before awaiting
    let node = {
        let guard = state.node.lock()
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.as_ref()
            .ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
            .clone()
    };

    node.send_direct_message(&body.recipient_node_id, &msg_bytes)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("P2P send failed: {}", e)))?;

    Ok(Json(json!({ "status": "sent_p2p" })))
}

#[derive(Deserialize)]
pub struct StartDmListenerBody {
    pub contact_node_id: String,
}

/// POST /api/chat/listen — no-op in relay mode, kept for backwards compatibility
pub async fn start_dm_listener(
    _state: State<Arc<AppState>>,
    Json(body): Json<StartDmListenerBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // In server relay mode the WebSocket connection itself is the listener.
    // Nothing to set up here.
    Ok(Json(json!({ "status": "ready", "contact": body.contact_node_id })))
}
