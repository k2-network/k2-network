use std::sync::Arc;
use axum::{extract::{State, Path}, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::state::AppState;

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

/// GET /api/contacts
pub async fn list_contacts(State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let contacts_guard = state.contacts.read().await;
    let cb = contacts_guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Contacts not initialized".to_string()))?;
    let contacts = cb.list().await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!(contacts)))
}

/// POST /api/contacts
pub async fn add_contact(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddContactBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let contacts_guard = state.contacts.read().await;
    let cb = contacts_guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Contacts not initialized".to_string()))?;
    let contact = cb.add(body.node_id, body.nickname, body.notes).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!(contact)))
}

/// DELETE /api/contacts/:nodeId
pub async fn remove_contact(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let contacts_guard = state.contacts.read().await;
    let cb = contacts_guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Contacts not initialized".to_string()))?;
    let removed = cb.remove(&node_id).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "removed": removed })))
}

/// PUT /api/contacts/:nodeId
pub async fn update_contact_nickname(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
    Json(body): Json<UpdateNicknameBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let contacts_guard = state.contacts.read().await;
    let cb = contacts_guard.as_ref().ok_or((StatusCode::BAD_REQUEST, "Contacts not initialized".to_string()))?;
    let updated = cb.update_nickname(&node_id, body.nickname).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "updated": updated })))
}

/// POST /api/contacts/:nodeId/ping
pub async fn ping_contact(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };
    let online = node.connect_to_contact(&node_id).await.is_ok();
    Ok(Json(json!({ "online": online })))
}
