//! WebSocket connection loop & multiplexer
//!
//! ### Architectural Overview
//! - **What it does**: Manages an individual WebSocket client session, multiplexing bidirectional RPC requests and server broadcast notifications over the socket.
//! - **How it does**: Splits the `WebSocket` into sender and receiver streams, selects concurrently over incoming socket text/binary messages and `manager.subscribe()` notification broadcast events, and transmits serialized JSON-RPC responses.
//! - **Where it comes from**: Called by `rpc::ws_handler` when upgrading HTTP connections.
//! - **Where it leads to**: Dispatches valid requests to `MethodRouter::dispatch()` and streams results and real-time events back to the client.

use super::auth::RpcAuthenticator;
use super::router::MethodRouter;
use crate::manager::DownloadManager;
use crate::models::RPCRequest;
use axum::extract::ws::{Message, WebSocket};
use futures::{sink::SinkExt, stream::StreamExt};
use std::sync::Arc;

/// Manages an individual WebSocket client session loop.
pub struct WebSocketHandler;

impl WebSocketHandler {
    /// Handles an individual WebSocket connection loop.
    ///
    /// Multiplexing mechanics:
    /// 1. Splits socket into `sender` (sink) and `receiver` (stream).
    /// 2. Subscribes to engine event broadcast notifications.
    /// 3. In a `tokio::select!` loop, handles:
    ///    - Incoming client messages: Parses JSON, validates authentication, routes through `MethodRouter::dispatch()`, and sends response.
    ///    - Outgoing broadcast notifications: Forwards engine lifecycle events directly over the socket.
    pub async fn handle(
        socket: WebSocket,
        manager: Arc<DownloadManager>,
        secret: Arc<Option<String>>,
    ) {
        let (mut sender, mut receiver) = socket.split();
        let mut notification_rx = manager.subscribe();

        loop {
            tokio::select! {
                // Handle incoming message from client
                Some(msg_res) = receiver.next() => {
                    if let Ok(msg) = msg_res {
                        let text = match msg {
                            Message::Text(t) => Some(t),
                            Message::Binary(b) => String::from_utf8(b).ok(),
                            Message::Close(_) => break,
                            _ => None,
                        };

                        if let Some(text) = text {
                            if let Ok(req) = serde_json::from_str::<RPCRequest>(&text) {
                                // Check secret token authentication
                                if !RpcAuthenticator::is_authenticated(&req, secret.as_deref()) {
                                    let error_res = RpcAuthenticator::unauthorized_response(&req.id);
                                    let _ = sender.send(Message::Text(serde_json::to_string(&error_res).unwrap())).await;
                                    continue;
                                }

                                // Route request to appropriate method handler
                                let response = MethodRouter::dispatch(req, manager.clone()).await;
                                if let Ok(json_res) = serde_json::to_string(&response) {
                                    let _ = sender.send(Message::Text(json_res)).await;
                                }
                            }
                        }
                    } else {
                        break;
                    }
                }
                // Handle engine lifecycle broadcast notification
                Ok(notification) = notification_rx.recv() => {
                    let _ = sender.send(Message::Text(notification)).await;
                }
            }
        }
    }
}
