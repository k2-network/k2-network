pub mod node;
pub mod contacts;
pub mod marketplace;
pub mod chat;
pub mod files;
pub mod ai;
pub mod qr;
pub mod tracker;

use std::sync::Arc;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use crate::state::AppState;

pub fn api_router() -> Router<Arc<AppState>> {
    Router::new()
        // Node
        .route("/init", post(node::init_node))
        .route("/node-id", get(node::get_my_node_id))
        // Contacts
        .route("/contacts", get(contacts::list_contacts))
        .route("/contacts", post(contacts::add_contact))
        .route("/contacts/:nodeId", delete(contacts::remove_contact))
        .route("/contacts/:nodeId", put(contacts::update_contact_nickname))
        .route("/contacts/:nodeId/ping", post(contacts::ping_contact))
        // Marketplace (P2P — dùng cho Tauri/multi-server)
        .route("/broadcast-delay", get(marketplace::get_broadcast_delay))
        .route("/topics/join", post(marketplace::join_topic))
        .route("/topics/broadcast", post(marketplace::broadcast_offer))
        .route("/topics/interest", post(marketplace::send_interest))
        .route("/topics/offers", get(marketplace::listen_offers))
        .route("/topics/listen", post(marketplace::start_listening))
        // Marketplace (Web matching engine — 1 server)
        .route("/offers", post(marketplace::post_offer))
        .route("/offers", get(marketplace::get_offers))
        // Chat
        .route("/chat/send", post(chat::send_chat_message))
        .route("/chat/send-p2p", post(chat::send_p2p_message))
        .route("/chat/listen", post(chat::start_dm_listener))
        // Files
        .route("/files/share", post(files::share_file))
        .route("/files/download", get(files::download_file))
        // AI
        .route("/classify-intent", post(ai::classify_intent))
        .route("/groq-chat", post(ai::groq_chat_with_tools))
        .route("/k2-endpoint", post(ai::classify_k2_endpoint))
        // QR
        .route("/qr-svg", post(qr::generate_qr_svg))
        // Tracker
        .route("/tracker/announce", post(tracker::announce_topic))
        .route("/tracker/peers", get(tracker::get_topic_peers))
}
