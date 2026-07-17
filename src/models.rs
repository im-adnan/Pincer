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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfoInner {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentInfo {
    #[serde(rename = "announceList")]
    pub announce_list: Vec<Vec<String>>,
    pub comment: Option<String>,
    #[serde(rename = "creationDate")]
    pub creation_date: Option<u64>,
    pub mode: String, // "single" or "multi"
    pub info: TorrentInfoInner,
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
    #[serde(rename = "uploadSpeed")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upload_speed: Option<String>,
    #[serde(rename = "workerProgress")]
    pub worker_progress: Vec<u64>,
    #[serde(rename = "fileType")]
    pub file_type: Option<String>,
    #[serde(rename = "isResumable")]
    pub is_resumable: Option<bool>,
    pub dir: String,
    pub files: Vec<FileData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bittorrent: Option<TorrentInfo>,
    #[serde(rename = "infoHash")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub info_hash: Option<String>,
    #[serde(rename = "numSeeders")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_seeders: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentResolveFile {
    pub index: usize,
    pub path: String,
    pub length: u64,
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
    #[serde(rename = "torrentFiles")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub torrent_files: Option<Vec<TorrentResolveFile>>,
}

/// Represents a serialized task saved to disk across sessions in `pincer.session`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionTask {
    pub id: String,
    pub filename: String,
    pub save_path: String,
    pub status: String,
    #[serde(default = "default_created_at")]
    pub created_at: u128,
    // The following fields are populated for complete/error tasks that no longer have a .download bundle
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub total_length: Option<u64>,
    #[serde(default)]
    pub completed_length: Option<u64>,
    #[serde(default)]
    pub file_type: Option<String>,
}

/// Represents the internal state of an active/paused download, stored within the .download bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleState {
    pub url: String,
    pub threads: usize,
    pub headers: Vec<String>,
    pub total_length: u64,
    pub completed_length: u64,
    #[serde(default)]
    pub worker_progress: Vec<u64>,
    #[serde(default)]
    pub chunk_size: u64,
    #[serde(default)]
    pub file_type: Option<String>,
}

fn default_created_at() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(std::time::Duration::from_millis(0))
        .as_millis()
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
