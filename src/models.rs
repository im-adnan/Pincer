use serde::{Deserialize, Serialize};

/// Represents an incoming JSON-RPC request from a client.
/// Follows the JSON-RPC 2.0 specification.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

/// Represents an outgoing JSON-RPC response back to the client.
/// Can contain either a `result` payload or an `error`.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCResponse<T> {
    pub id: String,
    pub jsonrpc: String,
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RPCError>,
}

/// Defines a standard JSON-RPC error payload.
#[derive(Debug, Serialize, Deserialize)]
pub struct RPCError {
    pub code: i32,
    pub message: String,
}

/// Contains all status metadata for an active, paused, or completed download task.
/// Includes progress, speed, and file information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStatus {
    pub gid: String,
    pub status: String,
    #[serde(rename = "totalLength")]
    pub total_length: String,
    #[serde(rename = "completedLength")]
    pub completed_length: String,
    #[serde(rename = "downloadSpeed")]
    pub download_speed: String,
    #[serde(rename = "workerProgress")]
    pub worker_progress: Vec<u64>,
    #[serde(rename = "fileType")]
    pub file_type: Option<String>,
    #[serde(rename = "isResumable")]
    pub is_resumable: Option<bool>,
    pub dir: String,
    pub files: Vec<FileData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    pub path: String,
    pub uris: Vec<FileUri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileUri {
    pub uri: String,
}

/// Global engine statistics including total active tasks and cumulative bandwidth usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStat {
    #[serde(rename = "downloadSpeed")]
    pub download_speed: String,
    #[serde(rename = "uploadSpeed")]
    pub upload_speed: String,
    #[serde(rename = "numActive")]
    pub num_active: String,
    #[serde(rename = "numWaiting")]
    pub num_waiting: String,
    #[serde(rename = "numStopped")]
    pub num_stopped: String,
    #[serde(rename = "numStoppedTotal")]
    pub num_stopped_total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RPCNotification {
    pub jsonrpc: String,
    pub method: String,
    pub params: Vec<NotificationParam>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationParam {
    pub gid: String,
}
/// The result of resolving a URL, providing metadata before the download begins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResponse {
    pub url: String,
    pub filename: Option<String>,
    #[serde(rename = "totalSize")]
    pub total_size: Option<i64>,
    #[serde(rename = "fileType")]
    pub file_type: Option<String>,
    #[serde(rename = "isResumable")]
    pub is_resumable: Option<bool>,
}

/// Represents a serialized task saved to disk across sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTask {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub threads: usize,
    pub headers: Vec<String>,
    pub status: String,
    pub total_length: u64,
    pub completed_length: u64,
    /// Per-worker bytes downloaded within their own chunk. Empty for fresh downloads.
    #[serde(default)]
    pub worker_progress: Vec<u64>,
    /// The original chunk size used when the download was started.
    /// Zero means the task is fresh and chunk size should be calculated from total_length.
    #[serde(default)]
    pub chunk_size: u64,
    #[serde(default)]
    pub file_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionData {
    pub tasks: Vec<SessionTask>,
    pub global_options: std::collections::HashMap<String, String>,
}

pub fn sanitize_filename(filename: &str) -> String {
    let raw_name = std::path::Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download.bin");

    if raw_name.is_empty() || raw_name == "." || raw_name == ".." {
        "download.bin".to_string()
    } else {
        raw_name.to_string()
    }
}

// Pincer Introspection Models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PincerUri {
    pub uri: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PincerFile {
    pub index: String,
    pub path: String,
    pub length: String,
    #[serde(rename = "completedLength")]
    pub completed_length: String,
    pub selected: String,
    pub uris: Vec<PincerUri>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PincerServerItem {
    pub uri: String,
    #[serde(rename = "currentUri")]
    pub current_uri: String,
    #[serde(rename = "downloadSpeed")]
    pub download_speed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PincerServer {
    pub index: String,
    pub servers: Vec<PincerServerItem>,
}
