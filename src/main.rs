mod handlers;
mod models;
mod peer_discovery;

use std::net::SocketAddr;

use axum::{
    routing::{get, post},
    Router,
};
use tracing::info;

use crate::models::AppState;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("p2p_node=info".parse().unwrap()),
        )
        .init();

    let address = std::env::var("P2P_ADDRESS").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port: u16 = std::env::var("P2P_PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);
    let hostname = std::env::var("P2P_HOSTNAME").unwrap_or_else(|_| {
        hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    });

    info!("Starting P2P node on {}:{} (hostname: {})", address, port, hostname);

    let state = AppState::new(address.clone(), port, hostname);
    let node_id = state.node_state.read().await.id.clone();
    info!("Node ID: {}", node_id);

    let app = Router::new()
        .route("/", get(handlers::index_handler))
        .route("/api/status", get(handlers::status_handler))
        .route("/api/peers", post(handlers::add_peer_handler))
        .route("/api/peers/remove", post(handlers::remove_peer_handler))
        .route("/api/peers/disconnect", post(handlers::disconnect_peer_handler))
        .route("/api/peers/connect", post(handlers::connect_peer_handler))
        .route("/api/peers/notify", post(handlers::notify_peer_handler))
        .route("/api/refresh", post(handlers::refresh_handler))
        .route("/api/handshake", post(handlers::handshake_handler))
        .route("/api/disconnect", post(handlers::disconnect_handler))
        .route("/api/disconnect-session", post(handlers::disconnect_session_handler))
        .with_state(state.clone());

    tokio::spawn(peer_discovery::start(state.clone()));

    let addr: SocketAddr = format!("{}:{}", address, port).parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    info!("Web interface available at http://{}", addr);

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
