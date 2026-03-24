use std::sync::Arc;
use axum::{extract::State, http::StatusCode, Json};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::{Utc, Duration};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::state::AppState;

#[derive(Serialize, Deserialize)]
struct AccessClaims {
    sub: String,
    exp: i64,
    iat: i64,
}

#[derive(Serialize, Deserialize)]
struct RefreshClaims {
    sub: String,
    jti: String,
    exp: i64,
    iat: i64,
}

#[derive(Deserialize)]
pub struct RegisterBody {
    pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginBody {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshBody {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct LogoutBody {
    pub refresh_token: String,
}

fn err(msg: &str) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "error": msg }))
}

fn make_access_token(user_id: &str, secret: &str) -> Option<String> {
    let now = Utc::now().timestamp();
    let claims = AccessClaims { sub: user_id.to_string(), iat: now, exp: now + 3600 };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).ok()
}

fn make_refresh_token(user_id: &str, jti: &str, secret: &str) -> Option<String> {
    let now = Utc::now().timestamp();
    let claims = RefreshClaims {
        sub: user_id.to_string(),
        jti: jti.to_string(),
        iat: now,
        exp: now + 30 * 24 * 3600,
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes())).ok()
}

pub async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let username = body.username.trim().to_string();
    let email = body.email.trim().to_lowercase();

    if username.len() < 3 {
        return Err((StatusCode::BAD_REQUEST, err("Tên người dùng tối thiểu 3 ký tự")));
    }
    if body.password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, err("Mật khẩu tối thiểu 8 ký tự")));
    }

    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = $1 OR username = $2"
    )
    .bind(&email).bind(&username)
    .fetch_one(&state.db).await.unwrap_or(0);

    if exists > 0 {
        return Err((StatusCode::CONFLICT, err("Email hoặc tên người dùng đã tồn tại")));
    }

    let user_id = Uuid::new_v4().to_string();
    let node_id = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let node_id = node_id[..64].to_string();
    let secret_key = Uuid::new_v4().to_string();

    let password_hash = hash(&body.password, DEFAULT_COST)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, err("Hash lỗi")))?;

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, node_id, secret_key) VALUES ($1::uuid, $2, $3, $4, $5, $6)"
    )
    .bind(&user_id).bind(&username).bind(&email).bind(&password_hash)
    .bind(&node_id).bind(&secret_key)
    .execute(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, err(&e.to_string())))?;

    let access_token = make_access_token(&user_id, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;
    let jti = Uuid::new_v4().to_string();
    let refresh_token = make_refresh_token(&user_id, &jti, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;

    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES ($1::uuid, $2::uuid, $3)")
        .bind(&jti).bind(&user_id)
        .bind(Utc::now() + Duration::days(30))
        .execute(&state.db).await.ok();

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "node_id": node_id,
        "username": username,
        "user_id": user_id,
    })))
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let email = body.email.trim().to_lowercase();

    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id::text, username, password_hash, node_id FROM users WHERE email = $1"
    )
    .bind(&email)
    .fetch_optional(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, err(&e.to_string())))?;

    let (user_id, username, password_hash, node_id) = row
        .ok_or((StatusCode::UNAUTHORIZED, err("Email hoặc mật khẩu không đúng")))?;

    let valid = verify(&body.password, &password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, err("Verify lỗi")))?;
    if !valid {
        return Err((StatusCode::UNAUTHORIZED, err("Email hoặc mật khẩu không đúng")));
    }

    let access_token = make_access_token(&user_id, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;
    let jti = Uuid::new_v4().to_string();
    let refresh_token = make_refresh_token(&user_id, &jti, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;

    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES ($1::uuid, $2::uuid, $3)")
        .bind(&jti).bind(&user_id)
        .bind(Utc::now() + Duration::days(30))
        .execute(&state.db).await.ok();

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "node_id": node_id,
        "username": username,
        "user_id": user_id,
    })))
}

pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let token_data = decode::<RefreshClaims>(
        &body.refresh_token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ).map_err(|_| (StatusCode::UNAUTHORIZED, err("Refresh token không hợp lệ hoặc đã hết hạn")))?;

    let claims = token_data.claims;

    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM refresh_tokens WHERE jti = $1::uuid AND user_id = $2::uuid"
    )
    .bind(&claims.jti).bind(&claims.sub)
    .fetch_one(&state.db).await.unwrap_or(0);

    if exists == 0 {
        return Err((StatusCode::UNAUTHORIZED, err("Refresh token đã bị thu hồi")));
    }

    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT username, node_id FROM users WHERE id = $1::uuid"
    )
    .bind(&claims.sub)
    .fetch_optional(&state.db).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, err(&e.to_string())))?
    .ok_or((StatusCode::UNAUTHORIZED, err("User không tồn tại")))?;

    let (username, node_id) = row;

    // Rotate token
    sqlx::query("DELETE FROM refresh_tokens WHERE jti = $1::uuid")
        .bind(&claims.jti).execute(&state.db).await.ok();

    let access_token = make_access_token(&claims.sub, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;
    let new_jti = Uuid::new_v4().to_string();
    let new_refresh = make_refresh_token(&claims.sub, &new_jti, &state.jwt_secret)
        .ok_or((StatusCode::INTERNAL_SERVER_ERROR, err("Token lỗi")))?;

    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES ($1::uuid, $2::uuid, $3)")
        .bind(&new_jti).bind(&claims.sub)
        .bind(Utc::now() + Duration::days(30))
        .execute(&state.db).await.ok();

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": new_refresh,
        "node_id": node_id,
        "username": username,
        "user_id": claims.sub,
    })))
}

pub async fn logout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LogoutBody>,
) -> Json<serde_json::Value> {
    if let Ok(token_data) = decode::<RefreshClaims>(
        &body.refresh_token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        sqlx::query("DELETE FROM refresh_tokens WHERE jti = $1::uuid")
            .bind(&token_data.claims.jti)
            .execute(&state.db).await.ok();
    }
    Json(serde_json::json!({ "status": "logged_out" }))
}
