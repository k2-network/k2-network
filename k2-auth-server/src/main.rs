use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use bcrypt::{hash, verify, DEFAULT_COST};
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use axum::http::{HeaderValue, Method};
use uuid::Uuid;

// ── JWT Claims ────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct AccessClaims {
    sub: String,   // user_id
    exp: i64,
    iat: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct RefreshClaims {
    sub: String,   // user_id
    jti: String,   // refresh token id (để revoke)
    exp: i64,
    iat: i64,
}

// ── App State ─────────────────────────────────────────────────────────────────

struct AppState {
    db: SqlitePool,
    jwt_secret: String,
}

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct RegisterBody {
    username: String,
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct LoginBody {
    email: String,
    password: String,
}

#[derive(Deserialize)]
struct RefreshBody {
    refresh_token: String,
}

#[derive(Deserialize)]
struct LogoutBody {
    refresh_token: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_access_token(user_id: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp();
    let claims = AccessClaims {
        sub: user_id.to_string(),
        iat: now,
        exp: now + 3600, // 1 giờ
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

fn make_refresh_token(user_id: &str, jti: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp();
    let claims = RefreshClaims {
        sub: user_id.to_string(),
        jti: jti.to_string(),
        iat: now,
        exp: now + 30 * 24 * 3600, // 30 ngày
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_bytes()))
}

fn error_json(msg: &str) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "error": msg }))
}

// ── Handlers ──────────────────────────────────────────────────────────────────

async fn register(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RegisterBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let username = body.username.trim().to_string();
    let email = body.email.trim().to_lowercase();

    if username.len() < 3 {
        return Err((StatusCode::BAD_REQUEST, error_json("Tên người dùng tối thiểu 3 ký tự")));
    }
    if body.password.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, error_json("Mật khẩu tối thiểu 8 ký tự")));
    }

    // Check duplicate email/username
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM users WHERE email = ? OR username = ?"
    )
    .bind(&email)
    .bind(&username)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if exists > 0 {
        return Err((StatusCode::CONFLICT, error_json("Email hoặc tên người dùng đã tồn tại")));
    }

    let user_id = Uuid::new_v4().to_string();
    // node_id: dùng UUID v4 encode hex 64 chars (giả lập iroh node id)
    let node_id = Uuid::new_v4().to_string().replace("-", "") + &Uuid::new_v4().to_string().replace("-", "");
    let node_id = &node_id[..64];

    let password_hash = hash(&body.password, DEFAULT_COST)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Hash lỗi")))?;

    sqlx::query(
        "INSERT INTO users (id, username, email, password_hash, node_id, created_at) VALUES (?, ?, ?, ?, ?, ?)"
    )
    .bind(&user_id)
    .bind(&username)
    .bind(&email)
    .bind(&password_hash)
    .bind(node_id)
    .bind(Utc::now().timestamp())
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, error_json(&e.to_string())))?;

    // Tạo tokens
    let access_token = make_access_token(&user_id, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    let jti = Uuid::new_v4().to_string();
    let refresh_token = make_refresh_token(&user_id, &jti, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    // Lưu refresh token
    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&jti)
        .bind(&user_id)
        .bind(Utc::now().timestamp() + 30 * 24 * 3600)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, error_json(&e.to_string())))?;

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "node_id": node_id,
        "username": username,
        "user_id": user_id,
    })))
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let email = body.email.trim().to_lowercase();

    let row = sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT id, username, password_hash, node_id FROM users WHERE email = ?"
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, error_json(&e.to_string())))?;

    let (user_id, username, password_hash, node_id) = row
        .ok_or((StatusCode::UNAUTHORIZED, error_json("Email hoặc mật khẩu không đúng")))?;

    let valid = verify(&body.password, &password_hash)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Verify lỗi")))?;

    if !valid {
        return Err((StatusCode::UNAUTHORIZED, error_json("Email hoặc mật khẩu không đúng")));
    }

    let access_token = make_access_token(&user_id, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    let jti = Uuid::new_v4().to_string();
    let refresh_token = make_refresh_token(&user_id, &jti, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&jti)
        .bind(&user_id)
        .bind(Utc::now().timestamp() + 30 * 24 * 3600)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, error_json(&e.to_string())))?;

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": refresh_token,
        "node_id": node_id,
        "username": username,
        "user_id": user_id,
    })))
}

async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshBody>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let token_data = decode::<RefreshClaims>(
        &body.refresh_token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|_| (StatusCode::UNAUTHORIZED, error_json("Refresh token không hợp lệ hoặc đã hết hạn")))?;

    let claims = token_data.claims;

    // Check token còn trong DB (chưa bị revoke)
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM refresh_tokens WHERE jti = ? AND user_id = ?"
    )
    .bind(&claims.jti)
    .bind(&claims.sub)
    .fetch_one(&state.db)
    .await
    .unwrap_or(0);

    if exists == 0 {
        return Err((StatusCode::UNAUTHORIZED, error_json("Refresh token đã bị thu hồi")));
    }

    // Lấy thông tin user
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT username, node_id FROM users WHERE id = ?"
    )
    .bind(&claims.sub)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, error_json(&e.to_string())))?
    .ok_or((StatusCode::UNAUTHORIZED, error_json("User không tồn tại")))?;

    let (username, node_id) = row;

    // Rotate: xóa token cũ, tạo token mới
    sqlx::query("DELETE FROM refresh_tokens WHERE jti = ?")
        .bind(&claims.jti)
        .execute(&state.db)
        .await.ok();

    let access_token = make_access_token(&claims.sub, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    let new_jti = Uuid::new_v4().to_string();
    let new_refresh = make_refresh_token(&claims.sub, &new_jti, &state.jwt_secret)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, error_json("Token lỗi")))?;

    sqlx::query("INSERT INTO refresh_tokens (jti, user_id, expires_at) VALUES (?, ?, ?)")
        .bind(&new_jti)
        .bind(&claims.sub)
        .bind(Utc::now().timestamp() + 30 * 24 * 3600)
        .execute(&state.db)
        .await.ok();

    Ok(Json(serde_json::json!({
        "access_token": access_token,
        "refresh_token": new_refresh,
        "node_id": node_id,
        "username": username,
        "user_id": claims.sub,
    })))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LogoutBody>,
) -> Json<serde_json::Value> {
    // Decode để lấy jti, bỏ qua lỗi
    if let Ok(token_data) = decode::<RefreshClaims>(
        &body.refresh_token,
        &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        sqlx::query("DELETE FROM refresh_tokens WHERE jti = ?")
            .bind(&token_data.claims.jti)
            .execute(&state.db)
            .await.ok();
    }
    Json(serde_json::json!({ "status": "logged_out" }))
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://k2_auth.db".to_string());
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| "k2-secret-change-in-production".to_string());

    // Tạo DB và migrate
    let db = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Failed to connect to SQLite");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT UNIQUE NOT NULL,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL,
            node_id TEXT NOT NULL,
            created_at INTEGER NOT NULL
        )"
    ).execute(&db).await.expect("Failed to create users table");

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS refresh_tokens (
            jti TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            expires_at INTEGER NOT NULL
        )"
    ).execute(&db).await.expect("Failed to create refresh_tokens table");

    let state = Arc::new(AppState { db, jwt_secret });

    let allowed_origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "https://k2team.xyz,https://www.k2team.xyz,http://localhost:5173".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let app = Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/refresh", post(refresh))
        .route("/api/auth/logout", post(logout))
        .layer(cors)
        .with_state(state);

    let addr = std::env::var("K2_AUTH_ADDR").unwrap_or_else(|_| "0.0.0.0:3002".to_string());
    println!("[K2 Auth Server] Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.expect("Failed to bind");
    axum::serve(listener, app).await.expect("Server error");
}
