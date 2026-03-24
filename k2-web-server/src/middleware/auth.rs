use std::sync::Arc;
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AccessClaims {
    pub sub: String, // user_id
    pub exp: i64,
    pub iat: i64,
}

/// Extractor: lấy claims từ Bearer token trong Authorization header
/// Dùng làm axum middleware — reject 401 nếu không có token hoặc token invalid/expired
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or((StatusCode::UNAUTHORIZED, "Missing or invalid Authorization header"))?;

    let claims = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))?
    .claims;

    // Inject claims vào request extensions để handler dùng nếu cần
    req.extensions_mut().insert(claims);

    Ok(next.run(req).await)
}
