//! Startup session loader
//!
//! ### Architectural Overview
//! - **What it does**: Restores persisted tasks, worker progress checkpoints, and global settings from `pincer.session` and `.download/state.json` on application startup.
//! - **How it does**: Reads saved `SessionData`, checks per-bundle `state.json` files for active chunk progress, restores global options, and populates `TaskControl` entries in the `tasks` registry.
//! - **Where it comes from**: Called by `manager.load_session()` from `src/main.rs` before accepting new commands or connections.
//! - **Where it leads to**: Re-populates the in-memory task database so paused, completed, and interrupted downloads are immediately accessible.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use super::persistence::SessionPersistence;
use super::state::TaskControl;
use crate::models::{BundleState, FileData, FileUri, TaskStatus};

/// Restores saved session state and task checkpoints on engine startup.
pub struct SessionLoader;

impl SessionLoader {
    /// Loads saved session data from disk into the running manager instance.
    ///
    /// Startup sequence:
    /// 1. Reads `pincer.session` from `~/.pincer/`.
    /// 2. Restores global options and bandwidth limits.
    /// 3. For each task, checks for `.download/state.json` to recover per-worker chunk progress.
    /// 4. Inserts reconstructed `TaskControl` instances into `tasks` registry.
    pub async fn restore_session(
        tasks: &RwLock<HashMap<String, TaskControl>>,
        global_options: &RwLock<HashMap<String, String>>,
        default_split: &Arc<AtomicU64>,
        current_limit: &Arc<AtomicU64>,
    ) {
        if let Some(session_data) = SessionPersistence::load() {
            {
                let mut global_opts = global_options.write().await;
                *global_opts = session_data.global_options;
                if let Some(split_str) = global_opts.get("default-split") {
                    if let Ok(split) = split_str.parse::<u64>() {
                        default_split.store(split, Ordering::Relaxed);
                    }
                }
                if let Some(limit_str) = global_opts.get("max-overall-download-limit") {
                    if let Ok(limit) = limit_str.parse::<u64>() {
                        current_limit.store(limit, Ordering::Relaxed);
                    }
                }
            }

            for task in &session_data.tasks {
                let mut opts = HashMap::new();
                opts.insert("dir".to_string(), task.save_path.clone());
                opts.insert("out".to_string(), task.filename.clone());

                let mut task_status = task.status.clone();
                let mut bundle_state_opt = None;

                let is_torrent = task.file_type.as_deref() == Some("torrent");
                if task_status != "complete" && task_status != "error" && !is_torrent {
                    let state_path =
                        format!("{}/{}.download/state.json", task.save_path, task.filename);
                    if let Ok(json_str) = std::fs::read_to_string(&state_path) {
                        if let Ok(b) = serde_json::from_str::<BundleState>(&json_str) {
                            bundle_state_opt = Some(b);
                        } else {
                            task_status = "error".to_string();
                        }
                    } else {
                        task_status = "error".to_string();
                    }
                }

                let (url, threads, headers, total_len, comp_len, wp, file_type) =
                    if let Some(b) = bundle_state_opt {
                        (
                            b.url,
                            b.threads,
                            b.headers,
                            b.total_length,
                            b.completed_length,
                            b.worker_progress,
                            b.file_type,
                        )
                    } else {
                        (
                            task.url.clone().unwrap_or_default(),
                            1,
                            Vec::new(),
                            task.total_length.unwrap_or(0),
                            task.completed_length.unwrap_or(0),
                            Vec::new(),
                            task.file_type.clone(),
                        )
                    };

                opts.insert("split".to_string(), threads.to_string());
                if !headers.is_empty() {
                    opts.insert("header".to_string(), headers.join("\n"));
                }

                let initial_status = TaskStatus {
                    gid: task.id.clone(),
                    status: task_status,
                    total_length: total_len.to_string(),
                    completed_length: comp_len.to_string(),
                    download_speed: "0".to_string(),
                    upload_speed: None,
                    worker_progress: wp,
                    file_type,
                    is_resumable: Some(true),
                    files: vec![FileData {
                        path: format!("{}/{}", task.save_path, task.filename),
                        uris: vec![FileUri { uri: url.clone() }],
                    }],
                    dir: task.save_path.clone(),
                    bittorrent: None,
                    info_hash: None,
                    num_seeders: None,
                    url: Some(url),
                };

                let mut tasks_guard = tasks.write().await;
                tasks_guard.insert(
                    task.id.clone(),
                    TaskControl::new(
                        initial_status,
                        CancellationToken::new(),
                        opts,
                        None,
                        task.created_at,
                    ),
                );
            }
            println!(
                "Session successfully loaded. Restored {} tasks.",
                session_data.tasks.len()
            );
        }
    }
}
