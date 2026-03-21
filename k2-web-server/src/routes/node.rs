use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::{extract::State, http::StatusCode, Json};
use serde_json::json;
use k2_core::K2Node;
use crate::state::{AppState, WsEvent};

/// POST /api/init — Initialize K2 node
pub async fn init_node(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Guard: already initialized
    {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        if let Some(ref node) = *guard {
            let node_id = node.my_id();
            let short_id = if node_id.len() > 10 {
                format!("{}...", &node_id[..10])
            } else {
                node_id
            };
            return Ok(Json(json!({ "node_id": short_id, "status": "existing" })));
        }
    }

    let node = K2Node::new().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create K2Node: {}", e)))?;

    let node_id = node.my_id();
    let short_id = if node_id.len() > 10 {
        format!("{}...", &node_id[..10])
    } else {
        node_id.clone()
    };

    // Initialize ContactBookDocs
    let mut contact_book = node.contact_book();
    contact_book.init().await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to init contacts: {}", e)))?;

    {
        let mut contacts_guard = state.contacts.write().await;
        *contacts_guard = Some(contact_book);
    }
    {
        let mut node_guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        *node_guard = Some(node);
    }

    // Spawn DM bridge: forward P2P direct messages to WebSocket clients
    let node_clone = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.as_ref().unwrap().clone()
    };
    let my_node_id = node_clone.my_id();
    if let Some(mut dm_rx) = node_clone.take_dm_receiver().await {
        let event_tx = state.event_tx.clone();
        tokio::spawn(async move {
            while let Some((sender_node_id, raw_bytes)) = dm_rx.recv().await {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let (sender_name, content) = if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw_bytes) {
                    (
                        v.get("sender_name").and_then(|x| x.as_str()).unwrap_or("Unknown").to_string(),
                        v.get("content").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    )
                } else {
                    ("Unknown".to_string(), String::from_utf8_lossy(&raw_bytes).to_string())
                };
                let payload = serde_json::json!({
                    "sender_node_id": sender_node_id,
                    "sender_name": sender_name,
                    "content": content,
                    "timestamp": timestamp,
                });
                let _ = event_tx.send(WsEvent::ChatMessage {
                    recipient_session_id: my_node_id.clone(),
                    payload,
                });
            }
        });
    }

    Ok(Json(json!({ "node_id": short_id, "status": "initialized" })))
}

/// GET /api/node-id — Get full node ID
pub async fn get_my_node_id(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let node = guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?;
    Ok(Json(json!({ "node_id": node.my_id() })))
}
