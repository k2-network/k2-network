mod state;
mod routes;
mod ws;
mod middleware;

use std::sync::Arc;
use std::net::SocketAddr;
use axum::{middleware as axum_middleware, routing::get, Router};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use axum::http::{HeaderValue, Method};

#[tokio::main]
async fn main() {
    println!("[K2 Web Server] Starting...");

    let (app_state, _initial_rx) = state::AppState::new().await;

    let allowed_origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://localhost:3000,https://k2team.xyz,https://www.k2team.xyz".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    // Rate limiter: burst 20, refill 1 request/5 giây (tối đa ~12/phút)
    let auth_governor_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(5)
            .burst_size(20)
            .finish()
            .expect("Failed to build rate limiter config"),
    );

    // Login/register có rate limit (chống brute force)
    let auth_router = axum::Router::new()
        .route("/api/auth/register", axum::routing::post(routes::auth::register))
        .route("/api/auth/login", axum::routing::post(routes::auth::login))
        .layer(GovernorLayer { config: Arc::clone(&auth_governor_conf) })
        .with_state(Arc::clone(&app_state));

    // Refresh/logout không rate limit — gọi tự động mỗi lần F5
    let session_router = axum::Router::new()
        .route("/api/auth/refresh", axum::routing::post(routes::auth::refresh))
        .route("/api/auth/logout", axum::routing::post(routes::auth::logout))
        .with_state(Arc::clone(&app_state));

    // Protected router với JWT middleware
    let protected = routes::protected_router()
        .route_layer(axum_middleware::from_fn_with_state(
            Arc::clone(&app_state),
            middleware::auth::require_auth,
        ));

    // Public router (không có auth routes — đã tách riêng với rate limit)
    let public = routes::public_router();

    let api = Router::new()
        .merge(public)
        .merge(protected)
        .with_state(Arc::clone(&app_state));

    let app = Router::new()
        .nest("/api", api)
        .merge(auth_router)
        .merge(session_router)
        .route("/ws", get(ws::ws_handler))
        .layer(cors)
        .with_state(Arc::clone(&app_state));

    let addr = std::env::var("K2_SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:3001".to_string());
    println!("[K2 Web Server] Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("Server error");
}
