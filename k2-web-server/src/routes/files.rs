use std::sync::Arc;
use axum::{
    extract::{Query, State, Multipart},
    http::{StatusCode, header},
    response::Response,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use directories::UserDirs;
use crate::state::AppState;

/// POST /api/files/share — accept multipart form, share bytes via iroh-blobs
pub async fn share_file(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::from("file");

    while let Some(field) = multipart.next_field().await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        if let Some(name) = field.file_name() {
            filename = name.to_string();
        }
        let data = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
        file_bytes = Some(data.to_vec());
    }

    let bytes = file_bytes.ok_or((StatusCode::BAD_REQUEST, "No file in request".to_string()))?;
    let ticket = node.share_bytes(&bytes, &filename).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({ "ticket": ticket, "filename": filename })))
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    pub ticket: String,
}

/// GET /api/files/download?ticket=X — download file and stream back
pub async fn download_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, (StatusCode, String)> {
    let node = {
        let guard = state.node.lock().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        guard.clone().ok_or((StatusCode::BAD_REQUEST, "Node not initialized".to_string()))?
    };

    let save_dir = get_download_dir();
    if !save_dir.exists() {
        std::fs::create_dir_all(&save_dir)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create download dir: {}", e)))?;
    }

    let filename = node.download_file(&query.ticket, &save_dir).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Download failed: {}", e)))?;

    let full_path = save_dir.join(&filename);
    let bytes = std::fs::read(&full_path)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read file: {}", e)))?;

    let disposition = format!("attachment; filename=\"{}\"", filename);
    let response = Response::builder()
        .header(header::CONTENT_DISPOSITION, disposition)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(axum::body::Body::from(bytes))
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(response)
}

fn get_download_dir() -> std::path::PathBuf {
    if let Some(user_dirs) = UserDirs::new() {
        user_dirs.download_dir()
            .unwrap_or(&std::path::PathBuf::from("."))
            .to_path_buf()
    } else {
        std::path::PathBuf::from(".")
    }
}
