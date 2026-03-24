pub mod event_bus;

use std::sync::Arc;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use crate::state::{AppState, WsEvent};

#[derive(Deserialize)]
pub struct WsQuery {
    pub session_id: Option<String>,
    pub node_id: Option<String>,
}

/// WebSocket handler at GET /ws?session_id=<uuid>&node_id=<iroh_node_id>
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let session_id = query.session_id.unwrap_or_default();
    let node_id = query.node_id.unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, state, session_id, node_id))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, session_id: String, node_id: String) {
    println!("[ws] client connected session_id={:?} node_id={:?}", session_id, node_id);

    // Lưu map để route chat messages:
    // - node_id (iroh) → session_id: dùng khi sender biết iroh node_id của recipient
    // - session_id → session_id: dùng khi recipient_node_id chính là session_id
    if !session_id.is_empty() {
        let mut map = state.node_to_session.write().await;
        if !node_id.is_empty() {
            map.insert(node_id.clone(), session_id.clone());
            println!("[ws] registered iroh={} -> session={}", node_id, session_id);
        }
        // Luôn map session_id → session_id để routing bằng session_id cũng work
        map.insert(session_id.clone(), session_id.clone());
        println!("[ws] registered session={} -> session={}", session_id, session_id);
    }

    let mut event_rx = state.event_tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Clone để dùng trong closure và sau khi spawn
    let session_id_for_task = session_id.clone();

    // Task: forward WsEvents to WebSocket client, filtering by session_id
    let send_task = tokio::spawn(async move {
        let session_id = session_id_for_task;
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    // For directed events, only forward to the intended recipient
                    let should_send = match &event {
                        WsEvent::ChatMessage { recipient_session_id, .. }
                        | WsEvent::FriendRequest { recipient_session_id, .. }
                        | WsEvent::FriendRequestResponse { recipient_session_id, .. } => {
                            session_id.is_empty() || recipient_session_id == &session_id
                        }
                        // Broadcast all other events to everyone
                        _ => true,
                    };

                    if should_send {
                        if let Ok(json) = serde_json::to_string(&event) {
                            if sender.send(Message::Text(json.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            }
        }
    });

    // Drain incoming messages (keep connection alive / handle close)
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }

    send_task.abort();

    // Xóa map khi disconnect
    {
        let mut map = state.node_to_session.write().await;
        if !node_id.is_empty() {
            map.remove(&node_id);
        }
        if !session_id.is_empty() {
            map.remove(&session_id);
        }
        println!("[ws] client disconnected node_id={} session_id={}", node_id, session_id);
    }
}
