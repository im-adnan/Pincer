//! RPC notification builder & broadcast
//!
//! ### Architectural Overview
//! - **What it does**: Constructs and broadcasts real-time JSON-RPC 2.0 notifications (`pin.onDownloadStart`, `pin.onDownloadPause`, `pin.onDownloadComplete`, `pin.onDownloadError`, `pin.onDownloadProgress`) to connected clients.
//! - **How it does**: Formats `RPCNotification` structs with task GID parameter arrays, serializes to JSON strings, and publishes them across Tokio `broadcast::Sender<String>`.
//! - **Where it comes from**: Called by task runners, lifecycle managers, and torrent orchestrators whenever task state transitions occur.
//! - **Where it leads to**: Pushes live websocket events to connected frontend clients and external listeners.

use crate::models::{NotificationParam, RPCNotification};
use tokio::sync::broadcast;

/// Constructs and emits real-time JSON-RPC 2.0 lifecycle notifications.
pub struct EventNotifier;

impl EventNotifier {
    /// Builds a serialized JSON-RPC notification payload for the given method and task GID.
    pub fn build(method: &str, gid: &str) -> String {
        let notification = RPCNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: vec![NotificationParam {
                gid: gid.to_string(),
            }],
        };
        serde_json::to_string(&notification).unwrap_or_default()
    }

    /// Emits a notification payload to all connected WebSocket subscribers.
    pub fn emit(tx: &broadcast::Sender<String>, method: &str, gid: &str) {
        let payload = Self::build(method, gid);
        let _ = tx.send(payload);
    }
}
