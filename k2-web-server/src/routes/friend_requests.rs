use std::sync::Arc;
use axum::{extract::{State, Path}, http::StatusCode, Json};
use axum::Extension;
use serde::Deserialize;
use serde_json::json;
use chrono::Utc;
use crate::state::{AppState, WsEvent};
use crate::middleware::auth::AccessClaims;

#[derive(Deserialize)]
pub struct SendRequestBody {
    pub to_node_id: String,
}

/// POST /api/friend-requests — gửi lời mời kết bạn
pub async fn send_request(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Json(body): Json<SendRequestBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    if body.to_node_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "to_node_id is required".to_string()));
    }

    // Lấy node_id và username của người gửi
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT node_id, username FROM users WHERE id = $1::uuid"
    )
    .bind(&claims.sub)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let (from_node_id, from_username) = row;

    if from_node_id == body.to_node_id {
        return Err((StatusCode::BAD_REQUEST, "Cannot send friend request to yourself".to_string()));
    }

    // Kiểm tra đã là bạn chưa
    let already_friend: bool = sqlx::query_scalar(
        "SELECT COUNT(*) > 0 FROM contacts WHERE user_id = $1 AND node_id = $2"
    )
    .bind(&claims.sub)
    .bind(&body.to_node_id)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if already_friend {
        return Err((StatusCode::CONFLICT, "Already friends".to_string()));
    }

    // Kiểm tra đã có lời mời pending chưa
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM friend_requests WHERE from_node_id = $1 AND to_node_id = $2 AND status = 'pending'"
    )
    .bind(&from_node_id)
    .bind(&body.to_node_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if existing.is_some() {
        return Err((StatusCode::CONFLICT, "Friend request already sent".to_string()));
    }

    let now = Utc::now().timestamp();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO friend_requests (from_user_id, from_node_id, from_username, to_node_id, status, created_at)
         VALUES ($1, $2, $3, $4, 'pending', $5)
         RETURNING id"
    )
    .bind(&claims.sub)
    .bind(&from_node_id)
    .bind(&from_username)
    .bind(&body.to_node_id)
    .bind(now)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Gửi WS event real-time tới người nhận
    let recipient_session_id = {
        let map = state.node_to_session.read().await;
        map.get(&body.to_node_id).cloned().unwrap_or_else(|| body.to_node_id.clone())
    };

    let payload = json!({
        "id": id,
        "from_node_id": from_node_id,
        "from_username": from_username,
        "created_at": now,
    });

    let _ = state.event_tx.send(WsEvent::FriendRequest {
        recipient_session_id,
        payload,
    });

    Ok(Json(json!({ "status": "sent", "id": id })))
}

/// GET /api/friend-requests/pending — lấy danh sách lời mời chờ xác nhận (gửi tới mình)
pub async fn get_pending(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Lấy node_id của user hiện tại
    let my_node_id: String = sqlx::query_scalar("SELECT node_id FROM users WHERE id = $1::uuid")
        .bind(&claims.sub)
        .fetch_one(&state.db)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let rows = sqlx::query_as::<_, (i64, String, String, String, i64)>(
        "SELECT id, from_node_id, from_username, status, created_at
         FROM friend_requests
         WHERE to_node_id = $1 AND status = 'pending'
         ORDER BY created_at DESC"
    )
    .bind(&my_node_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let requests: Vec<serde_json::Value> = rows.into_iter().map(|(id, from_node_id, from_username, status, created_at)| {
        json!({ "id": id, "from_node_id": from_node_id, "from_username": from_username, "status": status, "created_at": created_at })
    }).collect();

    Ok(Json(json!(requests)))
}

/// GET /api/friend-requests/sent — lấy danh sách lời mời đã gửi (để UI biết mình đã gửi rồi)
pub async fn get_sent(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let my_node_id: String = sqlx::query_scalar("SELECT node_id FROM users WHERE id = $1::uuid")
        .bind(&claims.sub)
        .fetch_one(&state.db)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let rows = sqlx::query_as::<_, (i64, String, String, i64)>(
        "SELECT id, to_node_id, status, created_at
         FROM friend_requests
         WHERE from_node_id = $1 AND status = 'pending'
         ORDER BY created_at DESC"
    )
    .bind(&my_node_id)
    .fetch_all(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let requests: Vec<serde_json::Value> = rows.into_iter().map(|(id, to_node_id, status, created_at)| {
        json!({ "id": id, "to_node_id": to_node_id, "status": status, "created_at": created_at })
    }).collect();

    Ok(Json(json!(requests)))
}

/// PUT /api/friend-requests/:id/accept — chấp nhận lời mời
pub async fn accept_request(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // Lấy node_id của người nhận (chính mình)
    let (my_node_id, my_username): (String, String) = sqlx::query_as(
        "SELECT node_id, username FROM users WHERE id = $1::uuid"
    )
    .bind(&claims.sub)
    .fetch_one(&state.db)
    .await
    .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    // Lấy thông tin request
    let req = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT from_user_id, from_node_id, from_username, status
         FROM friend_requests WHERE id = $1 AND to_node_id = $2"
    )
    .bind(id)
    .bind(&my_node_id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((StatusCode::NOT_FOUND, "Friend request not found".to_string()))?;

    let (from_user_id, from_node_id, from_username, status) = req;

    if status != "pending" {
        return Err((StatusCode::CONFLICT, "Request already processed".to_string()));
    }

    // Cập nhật status
    sqlx::query("UPDATE friend_requests SET status = 'accepted' WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let now = Utc::now().timestamp();

    // Thêm người gửi vào contacts của mình (người nhận)
    let _ = sqlx::query(
        "INSERT INTO contacts (user_id, node_id, nickname, notes, added_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, node_id) DO NOTHING"
    )
    .bind(&claims.sub)
    .bind(&from_node_id)
    .bind(&from_username)
    .bind("Added via friend request")
    .bind(now)
    .execute(&state.db)
    .await;

    // Thêm mình vào contacts của người gửi
    let _ = sqlx::query(
        "INSERT INTO contacts (user_id, node_id, nickname, notes, added_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, node_id) DO NOTHING"
    )
    .bind(&from_user_id)
    .bind(&my_node_id)
    .bind(&my_username)
    .bind("Added via friend request")
    .bind(now)
    .execute(&state.db)
    .await;

    // Thông báo real-time cho người gửi
    let sender_session_id = {
        let map = state.node_to_session.read().await;
        map.get(&from_node_id).cloned().unwrap_or_else(|| from_node_id.clone())
    };

    let _ = state.event_tx.send(WsEvent::FriendRequestResponse {
        recipient_session_id: sender_session_id,
        payload: json!({
            "id": id,
            "status": "accepted",
            "by_node_id": my_node_id,
            "by_username": my_username,
        }),
    });

    Ok(Json(json!({
        "status": "accepted",
        "contact": { "node_id": from_node_id, "nickname": from_username, "added_at": now }
    })))
}

/// PUT /api/friend-requests/:id/decline — từ chối lời mời
pub async fn decline_request(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Path(id): Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let my_node_id: String = sqlx::query_scalar("SELECT node_id FROM users WHERE id = $1::uuid")
        .bind(&claims.sub)
        .fetch_one(&state.db)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, "User not found".to_string()))?;

    let result = sqlx::query(
        "UPDATE friend_requests SET status = 'declined'
         WHERE id = $1 AND to_node_id = $2 AND status = 'pending'"
    )
    .bind(id)
    .bind(&my_node_id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err((StatusCode::NOT_FOUND, "Request not found or already processed".to_string()));
    }

    // Thông báo cho người gửi
    let from_node_id: Option<String> = sqlx::query_scalar(
        "SELECT from_node_id FROM friend_requests WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten();

    if let Some(from_node) = from_node_id {
        let sender_session = {
            let map = state.node_to_session.read().await;
            map.get(&from_node).cloned().unwrap_or_else(|| from_node.clone())
        };
        let _ = state.event_tx.send(WsEvent::FriendRequestResponse {
            recipient_session_id: sender_session,
            payload: json!({ "id": id, "status": "declined", "by_node_id": my_node_id }),
        });
    }

    Ok(Json(json!({ "status": "declined" })))
}
