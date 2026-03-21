mod state;
mod routes;
mod ws;

use std::sync::Arc;
use axum::{routing::get, Router};
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use axum::http::{HeaderValue, Method};

#[tokio::main]
async fn main() {
    println!("[K2 Web Server] Starting...");

    let (app_state, _initial_rx) = state::AppState::new();

    let allowed_origins: Vec<HeaderValue> = std::env::var("ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://localhost:3000,https://k2team.xyz,https://www.k2team.xyz".to_string())
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(allowed_origins))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers(Any);

    let app = Router::new()
        .nest("/api", routes::api_router())
        .route("/ws", get(ws::ws_handler))
        .layer(cors)
        .with_state(Arc::clone(&app_state));

    let addr = std::env::var("K2_SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:3001".to_string());
    println!("[K2 Web Server] Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect("Failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("Server error");
}
