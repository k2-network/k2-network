use std::sync::Arc;
use axum::{extract::{State, Path}, http::StatusCode, Json};
use axum::Extension;
use serde::Deserialize;
use serde_json::json;
use chrono::Utc;
use crate::state::AppState;
use crate::middleware::auth::AccessClaims;

#[derive(Deserialize)]
pub struct AddContactBody {
    pub node_id: String,
    pub nickname: String,
    pub notes: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateNicknameBody {
    pub nickname: String,
}

/// GET /api/contacts — list contacts for the authenticated user
pub async fn list_contacts(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let rows = sqlx::query_as::<_, (String, String, Option<String>, i64)>(
        "SELECT node_id, nickname, notes, added_at FROM contacts WHERE user_id = $1 ORDER BY added_at DESC"
    )
    .bind(&claims.sub)
    .fetch_all(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let contacts: Vec<serde_json::Value> = rows.into_iter().map(|(node_id, nickname, notes, added_at)| {
        json!({ "node_id": node_id, "nickname": nickname, "notes": notes, "added_at": added_at })
    }).collect();

    Ok(Json(json!(contacts)))
}

/// POST /api/contacts — add a contact for the authenticated user
pub async fn add_contact(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Json(body): Json<AddContactBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let now = Utc::now().timestamp();

    sqlx::query(
        "INSERT INTO contacts (user_id, node_id, nickname, notes, added_at)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (user_id, node_id) DO UPDATE SET
           nickname = EXCLUDED.nickname,
           notes = CASE WHEN contacts.notes IS NOT NULL THEN contacts.notes ELSE EXCLUDED.notes END"
    )
    .bind(&claims.sub)
    .bind(&body.node_id)
    .bind(&body.nickname)
    .bind(&body.notes)
    .bind(now)
    .execute(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "node_id": body.node_id,
        "nickname": body.nickname,
        "notes": body.notes,
        "added_at": now,
    })))
}

/// DELETE /api/contacts/:nodeId — remove a contact (only if owned by this user)
pub async fn remove_contact(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let result = sqlx::query(
        "DELETE FROM contacts WHERE user_id = $1 AND node_id = $2"
    )
    .bind(&claims.sub)
    .bind(&node_id)
    .execute(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "removed": result.rows_affected() > 0 })))
}

/// PUT /api/contacts/:nodeId — update nickname (only if owned by this user)
pub async fn update_contact_nickname(
    State(state): State<Arc<AppState>>,
    Extension(claims): Extension<AccessClaims>,
    Path(node_id): Path<String>,
    Json(body): Json<UpdateNicknameBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let result = sqlx::query(
        "UPDATE contacts SET nickname = $1 WHERE user_id = $2 AND node_id = $3"
    )
    .bind(&body.nickname)
    .bind(&claims.sub)
    .bind(&node_id)
    .execute(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "updated": result.rows_affected() > 0 })))
}

/// POST /api/contacts/:nodeId/ping — check if a node is online (public, no auth needed)
pub async fn ping_contact(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let online = {
        let map = state.node_to_session.read().await;
        map.contains_key(&node_id) || map.values().any(|s| s == &node_id)
    };
    Ok(Json(json!({ "online": online })))
}
