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
}

/// WebSocket handler at GET /ws?session_id=<node_id>
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> Response {
    let session_id = query.session_id.unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, state, session_id))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, session_id: String) {
    let mut event_rx = state.event_tx.subscribe();
    let (mut sender, mut receiver) = socket.split();

    // Task: forward WsEvents to WebSocket client, filtering by session_id
    let send_task = tokio::spawn(async move {
        loop {
            match event_rx.recv().await {
                Ok(event) => {
                    // For ChatMessage, only forward to the intended recipient
                    let should_send = match &event {
                        WsEvent::ChatMessage { recipient_session_id, .. } => {
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
}
