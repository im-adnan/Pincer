//! SessionTask, SessionData, BundleState
//!
//! ### Architectural Overview
//! - **What it does**: Defines on-disk serialization structures for persisting engine state (`~/.pincer/pincer.session`) and per-download staging bundles (`<filename>.download/state.json`).
//! - **How it does**: Implements `serde::Serialize` and `serde::Deserialize` for structured JSON files that survive engine shutdowns and crashes.
//! - **Where it comes from**: Managed by `manager::SessionPersistence` and `manager::SessionLoader`.
//! - **Where it leads to**: Enables seamless resume across application restarts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level session manifest written to `~/.pincer/pincer.session`.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionData {
    /// Array of serialized task records.
    pub tasks: Vec<SessionTask>,
    /// Global configuration options map at time of session save.
    pub global_options: HashMap<String, String>,
}

/// Serialized task entry saved in the global session manifest.
#[derive(Debug, Serialize, Deserialize)]
pub struct SessionTask {
    /// Unique task GID.
    pub id: String,
    /// Destination filename.
    pub filename: String,
    /// Target directory path.
    pub save_path: String,
    /// Saved execution state ("paused", "complete", "error").
    pub status: String,
    /// Epoch timestamp when the task was initially submitted.
    #[serde(default = "super::task::default_created_at")]
    pub created_at: u128,
    /// Source download URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Expected total byte length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_length: Option<u64>,
    /// Downloaded byte count checkpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_length: Option<u64>,
    /// File type string if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file_type: Option<String>,
}

/// Internal checkpoint state stored inside `<filename>.download/state.json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BundleState {
    /// Target download URL.
    pub url: String,
    /// Number of concurrent worker threads.
    pub threads: usize,
    /// Custom HTTP request headers.
    pub headers: Vec<String>,
    /// Total file length in bytes.
    pub total_length: u64,
    /// Total completed bytes across all chunks.
    pub completed_length: u64,
    /// Per-worker byte progress counters.
    pub worker_progress: Vec<u64>,
    /// Calculated byte length of individual chunks.
    pub chunk_size: u64,
    /// Detected MIME file type.
    pub file_type: Option<String>,
}
