use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RPCRequest {
    pub jsonrpc: String,
    pub id: String,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RPCResponse<T> {
    pub id: String,
    pub jsonrpc: String,
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RPCError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RPCError {
    pub code: i32,
    pub message: String,
}

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
