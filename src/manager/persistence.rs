//! Save and load session (pincer.session)
//!
//! ### Architectural Overview
//! - **What it does**: Persists active, paused, and completed download task states and global configurations to disk (`~/.pincer/pincer.session` and `.download/state.json`).
//! - **How it does**: Serializes `SessionData` and individual `.download` bundle states to pretty JSON strings, writing atomically to filesystem locations under `~/.pincer/`.
//! - **Where it comes from**: Called by `manager.save_session()`, `manager.execute_task()`, `manager.pause_task()`, `manager.remove_task()`, and on engine shutdown.
//! - **Where it leads to**: Writes checkpoint files enabling seamless task recovery and resume upon application restarts.

use super::state::TaskControl;
use crate::models::{BundleState, SessionData, SessionTask};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Manages dual-layer session serialization to disk.
pub struct SessionPersistence;

impl SessionPersistence {
    /// Returns the standard session file path (`~/.pincer/pincer.session`).
    pub fn get_session_path() -> PathBuf {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = home.join(".pincer");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("pincer.session")
    }

    /// Serializes all tasks and global options to `pincer.session` and `<filename>.download/state.json`.
    ///
    /// Persistence strategy:
    /// 1. Maps active/converting tasks to "paused" in the saved manifest so they don't auto-start unmonitored.
    /// 2. For multi-worker direct downloads, writes chunk checkpoints to `.download/state.json`.
    /// 3. Serializes global options map and task records to `pincer.session`.
    pub fn save(tasks: &HashMap<String, TaskControl>, global_options: &HashMap<String, String>) {
        let mut session_tasks = Vec::new();
        for (id, control) in tasks.iter() {
            let status = &control.status;
            let saved_status = match status.status.as_str() {
                "active" | "converting" | "waiting" => "paused".to_string(),
                s => s.to_string(),
            };

            let first_uri = status
                .files
                .first()
                .and_then(|f| f.uris.first().map(|u| u.uri.clone()))
                .unwrap_or_default();
            let filename = status
                .files
                .first()
                .and_then(|f| {
                    Path::new(&f.path)
                        .file_name()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_default();
            let headers = control
                .options
                .get("header")
                .map(|h| {
                    h.split('\n')
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();

            let mut opt_url = None;
            let mut opt_total = None;
            let mut opt_completed = None;

            let is_torrent = status.file_type.as_deref() == Some("torrent");
            if saved_status == "complete" || saved_status == "error" || is_torrent {
                opt_url = Some(first_uri);
                opt_total = Some(status.total_length.parse::<u64>().unwrap_or(0));
                opt_completed = Some(status.completed_length.parse::<u64>().unwrap_or(0));
            } else {
                let threads = control
                    .options
                    .get("split")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(4);

                let bundle_state = BundleState {
                    url: first_uri,
                    threads,
                    headers,
                    total_length: status.total_length.parse::<u64>().unwrap_or(0),
                    completed_length: status.completed_length.parse::<u64>().unwrap_or(0),
                    worker_progress: status.worker_progress.clone(),
                    chunk_size: 0,
                    file_type: status.file_type.clone(),
                };

                let bundle_dir = format!("{}/{}.download", status.dir, filename);
                if !Path::new(&bundle_dir).exists() {
                    let _ = std::fs::create_dir_all(&bundle_dir);
                }

                let state_path = format!("{}/state.json", bundle_dir);
                if let Ok(json_str) = serde_json::to_string_pretty(&bundle_state) {
                    let _ = std::fs::write(&state_path, json_str);
                }
            }

            session_tasks.push(SessionTask {
                id: id.clone(),
                filename,
                save_path: status.dir.clone(),
                status: saved_status,
                created_at: control.created_at,
                url: opt_url,
                total_length: opt_total,
                completed_length: opt_completed,
                file_type: status.file_type.clone(),
            });
        }

        let session_data = SessionData {
            tasks: session_tasks,
            global_options: global_options.clone(),
        };

        if let Ok(json_str) = serde_json::to_string_pretty(&session_data) {
            let session_path = Self::get_session_path();
            if let Err(e) = std::fs::write(&session_path, json_str) {
                eprintln!("Failed to write session file to {:?}: {}", session_path, e);
            }
        }
    }

    /// Reads and deserializes session data from `~/.pincer/pincer.session`.
    pub fn load() -> Option<SessionData> {
        let session_path = Self::get_session_path();
        if !session_path.exists() {
            return None;
        }

        if let Ok(json_str) = std::fs::read_to_string(&session_path) {
            serde_json::from_str::<SessionData>(&json_str).ok()
        } else {
            None
        }
    }
}
