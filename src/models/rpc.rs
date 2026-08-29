//! JSON-RPC 2.0 request/response/error/notification models
//!
//! ### Architectural Overview
//! - **What it does**: Defines standard JSON-RPC 2.0 wire protocol data structures for requests, responses, errors, and asynchronous broadcast notifications.
//! - **How it does**: Implements `serde::Serialize` and `serde::Deserialize` for strongly-typed translation between WebSocket JSON payloads and Rust types.
//! - **Where it comes from**: Used by `rpc::WebSocketHandler`, `rpc::MethodRouter`, and `manager::EventNotifier`.
//! - **Where it leads to**: Serializes messages to WebSocket clients and deserializes client invocations.

use serde::{Deserialize, Serialize};

/// Standard JSON-RPC 2.0 Request envelope received from WebSocket clients.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RPCRequest {
    /// JSON-RPC version string (must be "2.0").
    pub jsonrpc: String,
    /// Request identifier used to correlate requests with responses.
    pub id: String,
    /// Target RPC method to execute (e.g. "pin.addUri", "pin.pause").
    pub method: String,
    /// Optional parameter payload (JSON array or object).
    pub params: Option<serde_json::Value>,
}

/// Standard JSON-RPC 2.0 Response envelope sent back to WebSocket clients.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCResponse<T> {
    /// JSON-RPC version string ("2.0").
    pub jsonrpc: String,
    /// Correlated request ID matching the incoming request.
    pub id: String,
    /// Result payload if execution succeeded; omitted on error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    /// Error descriptor if execution failed; omitted on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RPCError>,
}

/// JSON-RPC 2.0 Error descriptor containing numeric code and human-readable message.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCError {
    /// Numeric error code (e.g. 1 for Unauthorized, -32601 for Method Not Found).
    pub code: i32,
    /// Descriptive error message.
    pub message: String,
}

/// Asynchronous JSON-RPC 2.0 notification broadcast to all connected WebSocket clients.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCNotification {
    /// JSON-RPC version string ("2.0").
    pub jsonrpc: String,
    /// Event name (e.g. "pin.onDownloadStart", "pin.onDownloadComplete").
    pub method: String,
    /// Event parameter array containing task GID references.
    pub params: Vec<NotificationParam>,
}

/// Parameter entry for JSON-RPC broadcast notifications containing the associated task GID.
#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationParam {
    /// The global task identifier (GID) that triggered the event.
    pub gid: String,
}
