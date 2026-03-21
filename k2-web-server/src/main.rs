mod state;
mod routes;
mod ws;

use std::sync::Arc;
use axum::{routing::get, Router};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    println!("[K2 Web Server] Starting...");

    let (app_state, _initial_rx) = state::AppState::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
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
