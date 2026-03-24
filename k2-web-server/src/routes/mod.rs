pub mod node;
pub mod contacts;
pub mod friend_requests;
pub mod marketplace;
pub mod chat;
pub mod files;
pub mod ai;
pub mod qr;
pub mod tracker;
pub mod auth;

use std::sync::Arc;
use axum::{
    routing::{delete, get, post, put},
    Router,
};
use crate::state::AppState;

/// Routes không cần JWT — guest và auth đều dùng được
/// Auth routes được mount riêng trong main.rs với rate limiting
pub fn public_router() -> Router<Arc<AppState>> {
    Router::new()
        // Node init — cần trước khi có token
        .route("/init", post(node::init_node))
        .route("/node-id", get(node::get_my_node_id))
        // Tracker — ai cũng announce/query được
        .route("/tracker/announce", post(tracker::announce_topic))
        .route("/tracker/announce", delete(tracker::leave_topic))
        .route("/tracker/peers", get(tracker::get_topic_peers))
        .route("/tracker/subtopic-stats", get(tracker::get_subtopic_stats))
        // Marketplace — guest cũng post/get offer được
        .route("/offers", post(marketplace::post_offer))
        .route("/offers", get(marketplace::get_offers))
        .route("/offers", delete(marketplace::cancel_offer))
        .route("/broadcast-delay", get(marketplace::get_broadcast_delay))
        .route("/topics/join", post(marketplace::join_topic))
        .route("/topics/broadcast", post(marketplace::broadcast_offer))
        .route("/topics/interest", post(marketplace::send_interest))
        .route("/topics/offers", get(marketplace::listen_offers))
        .route("/topics/listen", post(marketplace::start_listening))
        // Ping — online status check, no auth needed
        .route("/contacts/:nodeId/ping", post(contacts::ping_contact))
        // QR
        .route("/qr-svg", post(qr::generate_qr_svg))
        // AI — cho phép guest dùng (key lấy từ env)
        .route("/classify-intent", post(ai::classify_intent))
        .route("/groq-chat", post(ai::groq_chat_with_tools))
        .route("/k2-endpoint", post(ai::classify_k2_endpoint))
        .route("/settings/groq-key", get(ai::check_groq_api_key))
        // Chat history — public vì AI chat không cần auth
        .route("/chat/messages", post(chat::save_chat_messages))
        .route("/chat/history", get(chat::get_chat_history))
        .route("/chat/history", delete(chat::clear_chat_history))
        // File upload/serve cho chat
        .route("/chat/upload", post(chat::upload_chat_file))
        .route("/chat/files/:file_id", get(chat::serve_chat_file))
}

/// Routes yêu cầu JWT — chỉ user đăng nhập mới dùng được
pub fn protected_router() -> Router<Arc<AppState>> {
    Router::new()
        // Node ID cố định per-user
        .route("/user/node-id", get(node::get_user_node_id))
        // Contacts
        .route("/contacts", get(contacts::list_contacts))
        .route("/contacts", post(contacts::add_contact))
        .route("/contacts/:nodeId", delete(contacts::remove_contact))
        .route("/contacts/:nodeId", put(contacts::update_contact_nickname))
        // Friend Requests
        .route("/friend-requests", post(friend_requests::send_request))
        .route("/friend-requests/pending", get(friend_requests::get_pending))
        .route("/friend-requests/sent", get(friend_requests::get_sent))
        .route("/friend-requests/:id/accept", put(friend_requests::accept_request))
        .route("/friend-requests/:id/decline", put(friend_requests::decline_request))
        // Chat
        .route("/chat/send", post(chat::send_chat_message))
        .route("/chat/send-p2p", post(chat::send_p2p_message))
        .route("/chat/listen", post(chat::start_dm_listener))
        // Files
        .route("/files/share", post(files::share_file))
        .route("/files/download", get(files::download_file))
        // Settings — chỉ auth user mới lưu Groq key
        .route("/settings/groq-key", post(ai::save_groq_api_key))
}
