//! Server startup & WebSocket upgrade
//!
//! ### Architectural Overview
//! - **What it does**: Binds the JSON-RPC TCP listener, serves WebSocket connections on `/jsonrpc`, and manages server lifecycle.
//! - **How it does**: Initializes an `axum::Router`, injects the `Arc<DownloadManager>` and optional secret token layers, binds to `0.0.0.0:<port>`, and upgrades incoming HTTP connections to WebSocket streams.
//! - **Where it comes from**: Called by `cli::CliDispatcher::parse_and_dispatch()` when running in RPC server or daemon mode.
//! - **Where it leads to**: Delegates accepted WebSocket connections to `WebSocketHandler::handle()` to process bidirectional RPC messages.

pub mod auth;
pub mod handlers;
pub mod router;
pub mod socket;

use axum::{extract::ws::WebSocketUpgrade, response::Response, routing::get, Extension, Router};
use std::sync::Arc;
use tokio::sync::broadcast;

// Re-export RPC server components
pub use auth::RpcAuthenticator;
pub use router::MethodRouter;
pub use socket::WebSocketHandler;

use crate::manager::DownloadManager;

/// Starts the Axum WebSocket JSON-RPC 2.0 server.
///
/// Setup stages:
/// 1. Builds Axum router with route `/jsonrpc` handling WebSocket upgrades.
/// 2. Injects shared `DownloadManager` and optional secret token via `Extension` layers.
/// 3. Binds TCP listener on `0.0.0.0:<port>`.
/// 4. Serves incoming connections asynchronously.
pub async fn start_server(
    manager: Arc<DownloadManager>,
    _rx: broadcast::Receiver<String>,
    port: u16,
    rpc_secret: Option<String>,
) {
    let secret = Arc::new(rpc_secret);
    let app = Router::new()
        .route("/jsonrpc", get(ws_handler))
        .layer(Extension(manager))
        .layer(Extension(secret));

    let addr = format!("0.0.0.0:{}", port);
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "Error: Failed to bind to port {}: {}. The port might already be in use by another instance or service.",
                port, e
            );
            std::process::exit(1);
        }
    };
    println!("Pincer listening on ws://{}/jsonrpc", addr);

    if let Err(e) = axum::serve(listener, app).await {
        eprintln!("RPC Server Error: {}", e);
    }
    println!("RPC Server has shut down.");
}

/// Upgrades incoming HTTP GET requests on `/jsonrpc` to WebSocket connections.
async fn ws_handler(
    ws: WebSocketUpgrade,
    Extension(manager): Extension<Arc<DownloadManager>>,
    Extension(secret): Extension<Arc<Option<String>>>,
) -> Response {
    ws.on_upgrade(move |socket| WebSocketHandler::handle(socket, manager, secret))
}
