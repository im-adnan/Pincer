//! Task registration spawner
//!
//! ### Architectural Overview
//! - **What it does**: Initializes, formats, and registers direct download tasks into the central in-memory task registry.
//! - **How it does**: Generates collision-free filenames via `UniqueNameGenerator::generate_unique()`, creates cancellation tokens, constructs initial `TaskStatus` structures, and inserts `TaskControl` instances into the `tasks` write lock.
//! - **Where it comes from**: Called by `manager.spawn_task()`.
//! - **Where it leads to**: Adds new tasks in `waiting` state, saving session state and triggering `schedule_tasks()`.

use super::state::TaskControl;
use super::unique_name::UniqueNameGenerator;
use crate::models::{default_created_at, FileData, FileUri, TaskStatus};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

/// Prepares and registers new download task structures in the manager's memory.
pub struct TaskSpawner;

impl TaskSpawner {
    /// Prepares and inserts a new download task into the manager registry.
    ///
    /// Registration sequence:
    /// 1. Ensures destination filename is unique and non-colliding via `UniqueNameGenerator`.
    /// 2. Allocates new `CancellationToken` for cooperative cancellation.
    /// 3. Populates initial `TaskStatus` with "waiting" status, zero speeds, and URI mappings.
    /// 4. Inserts `TaskControl` into `tasks` map.
    #[allow(clippy::too_many_arguments)]
    pub async fn register_task(
        id: String,
        urls: Vec<String>,
        filename: &str,
        dir: String,
        threads: usize,
        resume_offset: u64,
        headers: Vec<String>,
        expected_hash: Option<String>,
        tasks: &RwLock<HashMap<String, TaskControl>>,
    ) {
        let unique_filename = {
            let tasks_guard = tasks.read().await;
            UniqueNameGenerator::generate_unique(filename, Some(&id), &tasks_guard)
        };

        let token = CancellationToken::new();
        let completed_length = if resume_offset > 0 {
            resume_offset.to_string()
        } else {
            "0".to_string()
        };

        let mut initial_status = TaskStatus {
            gid: id.clone(),
            status: "waiting".to_string(),
            total_length: "0".to_string(),
            completed_length,
            download_speed: "0".to_string(),
            upload_speed: None,
            worker_progress: vec![0; threads],
            file_type: None,
            is_resumable: None,
            files: vec![FileData {
                path: format!("{}/{}", dir, unique_filename),
                uris: urls.iter().map(|u| FileUri { uri: u.clone() }).collect(),
            }],
            dir: dir.clone(),
            bittorrent: None,
            info_hash: None,
            num_seeders: None,
            url: urls.first().cloned(),
        };

        {
            let tasks_guard = tasks.read().await;
            if let Some(existing) = tasks_guard.get(&id) {
                initial_status.total_length = existing.status.total_length.clone();
                initial_status.file_type = existing.status.file_type.clone();
                initial_status.is_resumable = existing.status.is_resumable;

                let mut wp = existing.status.worker_progress.clone();
                if wp.len() == threads {
                    initial_status.worker_progress = wp;
                } else {
                    wp.resize(threads, 0);
                    initial_status.worker_progress = wp;
                }
            }
        }

        let mut opts = HashMap::new();
        opts.insert("dir".to_string(), dir);
        opts.insert("out".to_string(), unique_filename);
        opts.insert("split".to_string(), threads.to_string());
        if !headers.is_empty() {
            opts.insert("header".to_string(), headers.join("\n"));
        }

        let mut tasks_guard = tasks.write().await;
        tasks_guard.insert(
            id,
            TaskControl::new(
                initial_status,
                token,
                opts,
                expected_hash,
                default_created_at(),
            ),
        );
    }
}
