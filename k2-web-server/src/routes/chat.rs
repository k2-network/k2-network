use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::path::PathBuf;
use axum::{extract::{State, Query, Multipart, Path}, http::{StatusCode, header}, response::Response, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use crate::state::{AppState, WsEvent};

fn upload_dir() -> PathBuf {
    std::env::var("UPLOAD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/k2_uploads"))
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Send (relay) ───────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SendChatBody {
    pub sender_session_id: String,
    pub recipient_node_id: String,
    pub sender_name: String,
    pub content: String,
    pub sender_node_id: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
}

/// POST /api/chat/send
/// Server relay: receives message from sender, forwards via WS broadcast to recipient.
/// Also persists the message in chat_messages.
pub async fn send_chat_message(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SendChatBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let timestamp = now_ms();

    let sender_node_id = body.sender_node_id.as_deref()
        .unwrap_or(&body.sender_session_id);

    let payload = json!({
        "sender_node_id": sender_node_id,
        "sender_session_id": body.sender_session_id,
        "sender_name": body.sender_name,
        "content": body.content,
        "timestamp": timestamp,
        "reply_to_content": body.reply_to_content,
        "reply_to_sender": body.reply_to_sender,
    });

    // Lookup session_id của recipient từ node_id
    let recipient_session_id = {
        let map = state.node_to_session.read().await;
        map.get(&body.recipient_node_id).cloned().unwrap_or_else(|| body.recipient_node_id.clone())
    };

    println!("[chat/send] from={} to_node={} to_session={} content={:?}",
        body.sender_session_id, body.recipient_node_id, recipient_session_id, body.content);

    // Emit to recipient's WebSocket
    let result = state.event_tx.send(WsEvent::ChatMessage {
        recipient_session_id,
        payload,
    });

    println!("[chat/send] broadcast result: receivers={:?}", result);

    // Persist P2P message — conversation_id = sorted pair of node_ids
    let mut nodes = vec![sender_node_id.to_string(), body.recipient_node_id.clone()];
    nodes.sort();
    let conversation_id = format!("p2p_{}", nodes.join("_"));

    // Save for sender
    let _ = sqlx::query(
        "INSERT INTO chat_messages (session_id, conversation_id, role, content, sender_name, reply_to_content, reply_to_sender, created_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
    )
    .bind(&body.sender_session_id)
    .bind(&conversation_id)
    .bind("user")
    .bind(&body.content)
    .bind(&body.sender_name)
    .bind(&body.reply_to_content)
    .bind(&body.reply_to_sender)
    .bind(timestamp)
    .execute(&state.db)
    .await;

    // Always save for recipient (use node_id as session_id fallback — works for auth users where session_id == node_id)
    let recip_session = {
        let map = state.node_to_session.read().await;
        map.get(&body.recipient_node_id)
            .cloned()
            .unwrap_or_else(|| body.recipient_node_id.clone())
    };
    if recip_session != body.sender_session_id {
        let _ = sqlx::query(
            "INSERT INTO chat_messages (session_id, conversation_id, role, content, sender_name, reply_to_content, reply_to_sender, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"
        )
        .bind(&recip_session)
        .bind(&conversation_id)
        .bind("user")
        .bind(&body.content)
        .bind(&body.sender_name)
        .bind(&body.reply_to_content)
        .bind(&body.reply_to_sender)
        .bind(timestamp)
        .execute(&state.db)
        .await;
    }

    Ok(Json(json!({ "status": "sent" })))
}

// ── Send P2P ──────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SendP2pBody {
    pub recipient_node_id: String,
    pub sender_session_id: String,
    pub sender_name: String,
    pub content: String,
}

/// POST /api/chat/send-p2p
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

// ── Listen (no-op) ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct StartDmListenerBody {
    pub contact_node_id: String,
}

/// POST /api/chat/listen — no-op in relay mode, kept for backwards compatibility
pub async fn start_dm_listener(
    _state: State<Arc<AppState>>,
    Json(body): Json<StartDmListenerBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(json!({ "status": "ready", "contact": body.contact_node_id })))
}

// ── Save AI chat messages ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveMessageItem {
    pub role: String,
    pub content: String,
    pub sender_name: Option<String>,
}

#[derive(Deserialize)]
pub struct SaveMessagesBody {
    pub session_id: String,
    /// 'ai' for AI assistant chat, or 'p2p_<nodeA>_<nodeB>' for P2P
    pub conversation_id: String,
    pub messages: Vec<SaveMessageItem>,
}

/// POST /api/chat/messages — save one or more messages (AI or P2P)
pub async fn save_chat_messages(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SaveMessagesBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if body.session_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "session_id is required".to_string()));
    }
    let timestamp = now_ms();

    for msg in &body.messages {
        sqlx::query(
            "INSERT INTO chat_messages (session_id, conversation_id, role, content, sender_name, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)"
        )
        .bind(&body.session_id)
        .bind(&body.conversation_id)
        .bind(&msg.role)
        .bind(&msg.content)
        .bind(&msg.sender_name)
        .bind(timestamp)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }

    Ok(Json(json!({ "status": "saved", "count": body.messages.len() })))
}

// ── Clear chat history ────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClearHistoryQuery {
    pub session_id: String,
    pub conversation_id: String,
}

/// DELETE /api/chat/history?session_id=xxx&conversation_id=p2p_...
/// Xóa toàn bộ tin nhắn của một conversation (chỉ phía người gọi).
pub async fn clear_chat_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ClearHistoryQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if params.session_id.is_empty() || params.conversation_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "session_id and conversation_id are required".to_string()));
    }

    let result = sqlx::query(
        "DELETE FROM chat_messages WHERE session_id = $1 AND conversation_id = $2"
    )
    .bind(&params.session_id)
    .bind(&params.conversation_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "status": "cleared", "deleted": result.rows_affected() })))
}

// ── Get chat history ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct GetHistoryQuery {
    pub session_id: String,
    pub conversation_id: Option<String>,
    pub limit: Option<i64>,
}

#[derive(sqlx::FromRow, Serialize)]
struct ChatMessageRow {
    pub id: i64,
    pub role: String,
    pub content: String,
    pub sender_name: Option<String>,
    pub reply_to_content: Option<String>,
    pub reply_to_sender: Option<String>,
    pub created_at: i64,
}

/// GET /api/chat/history?session_id=xxx&conversation_id=ai&limit=50
/// Returns messages in chronological order (oldest first).
pub async fn get_chat_history(
    State(state): State<Arc<AppState>>,
    Query(params): Query<GetHistoryQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if params.session_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "session_id is required".to_string()));
    }
    let conversation_id = params.conversation_id.as_deref().unwrap_or("ai");
    let limit = params.limit.unwrap_or(50).min(200);

    // P2P conversations: query by conversation_id only (both sides' messages)
    // AI chat: filter by session_id to separate users
    let rows = if conversation_id.starts_with("p2p_") {
        sqlx::query_as::<_, ChatMessageRow>(
            "SELECT id, role, content, sender_name, reply_to_content, reply_to_sender, created_at
             FROM chat_messages
             WHERE conversation_id = $1
             ORDER BY created_at DESC, id DESC
             LIMIT $2"
        )
        .bind(conversation_id)
        .bind(limit * 2) // fetch more to allow deduplication
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    } else {
        sqlx::query_as::<_, ChatMessageRow>(
            "SELECT id, role, content, sender_name, reply_to_content, reply_to_sender, created_at
             FROM chat_messages
             WHERE session_id = $1 AND conversation_id = $2
             ORDER BY created_at DESC, id DESC
             LIMIT $3"
        )
        .bind(&params.session_id)
        .bind(conversation_id)
        .bind(limit)
        .fetch_all(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    };

    // Deduplicate (two copies saved per P2P message) by (sender_name, content, created_at)
    let mut seen = std::collections::HashSet::new();
    let rows: Vec<_> = rows.into_iter().filter(|r| {
        let key = format!("{}\x00{}\x00{}", r.sender_name.as_deref().unwrap_or(""), r.content, r.created_at);
        seen.insert(key)
    }).take(limit as usize).collect();

    // Reverse to chronological order (oldest → newest)
    let messages: Vec<_> = rows.into_iter().rev().map(|r| json!({
        "id": r.id.to_string(),
        "role": r.role,
        "content": r.content,
        "sender_name": r.sender_name,
        "reply_to_content": r.reply_to_content,
        "reply_to_sender": r.reply_to_sender,
        "created_at": r.created_at,
    })).collect();

    Ok(Json(json!({ "messages": messages })))
}

// ── File Upload ────────────────────────────────────────────────────────────────

const MAX_UPLOAD_BYTES: usize = 50 * 1024 * 1024; // 50 MB

/// POST /api/chat/upload — lưu file vào disk, trả về URL public
pub async fn upload_chat_file(
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let dir = upload_dir();
    std::fs::create_dir_all(&dir)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create upload dir: {e}")))?;

    while let Some(field) = multipart.next_field().await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let filename = field.file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "file".to_string());
        let content_type = field.content_type()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let data = field.bytes().await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        if data.len() > MAX_UPLOAD_BYTES {
            return Err((StatusCode::PAYLOAD_TOO_LARGE, "File vượt quá giới hạn 50MB".to_string()));
        }

        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| format!(".{s}"))
            .unwrap_or_default();
        let file_id = format!("{}{}", Uuid::new_v4(), ext);
        let file_path = dir.join(&file_id);

        std::fs::write(&file_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to save file: {e}")))?;

        return Ok(Json(json!({
            "file_id": file_id,
            "url": format!("/api/chat/files/{}", file_id),
            "filename": filename,
            "size": data.len(),
            "mime_type": content_type,
        })));
    }

    Err((StatusCode::BAD_REQUEST, "Không tìm thấy file trong request".to_string()))
}

/// GET /api/chat/files/:file_id — serve file đã upload
pub async fn serve_chat_file(
    Path(file_id): Path<String>,
) -> Result<Response, (StatusCode, String)> {
    // Chặn path traversal
    if file_id.contains('/') || file_id.contains('\\') || file_id.contains("..") {
        return Err((StatusCode::BAD_REQUEST, "Invalid file id".to_string()));
    }

    let file_path = upload_dir().join(&file_id);
    let data = std::fs::read(&file_path)
        .map_err(|_| (StatusCode::NOT_FOUND, "File not found".to_string()))?;

    let ext = std::path::Path::new(&file_id)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let mime = match ext.to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png"          => "image/png",
        "gif"          => "image/gif",
        "webp"         => "image/webp",
        "svg"          => "image/svg+xml",
        "mp4"          => "video/mp4",
        "webm"         => "video/webm",
        "pdf"          => "application/pdf",
        "txt"          => "text/plain; charset=utf-8",
        "zip"          => "application/zip",
        _              => "application/octet-stream",
    };

    let response = Response::builder()
        .header(header::CONTENT_TYPE, mime)
        .header(header::CACHE_CONTROL, "public, max-age=86400")
        .body(axum::body::Body::from(data))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}
