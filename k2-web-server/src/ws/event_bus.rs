use crate::state::WsEvent;
use tokio::sync::broadcast;

/// Create a new broadcast channel for WebSocket events
pub fn create_channel() -> (broadcast::Sender<WsEvent>, broadcast::Receiver<WsEvent>) {
    broadcast::channel(256)
}
