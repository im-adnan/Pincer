//! Worker engine runner & progress updates
//!
//! ### Architectural Overview
//! - **What it does**: Spawns and supervises background asynchronous `engine::DownloadTask` executions, streams chunk progress updates, computes smoothed download speeds, and finalizes completed downloads.
//! - **How it does**: Spawns Tokio tasks running `DownloadTask::start()`, receives `(worker_id, bytes_chunk)` channel events to update completed byte counts and speeds, verifies hashes via `TaskPostProcessor`, and emits lifecycle broadcast events.
//! - **Where it comes from**: Called by `manager.execute_task()` when tasks are scheduled for active execution.
//! - **Where it leads to**: Writes data to disk, transitions task state to `complete` or `error`, and broadcasts progress/completion notifications over WebSocket channels.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use super::execution::TaskPostProcessor;
use super::notifier::EventNotifier;
use super::persistence::SessionPersistence;
use super::state::TaskControl;
use crate::engine::DownloadTask;

/// Supervises active background downloads, aggregating progress and executing finalization routines.
pub struct TaskRunner;

impl TaskRunner {
    /// Increments downloaded chunk count and updates real-time speed.
    pub async fn update_progress(
        id: &str,
        worker_id: usize,
        downloaded_chunk: u64,
        tasks: &RwLock<HashMap<String, TaskControl>>,
    ) {
        let mut tasks_guard = tasks.write().await;
        if let Some(control) = tasks_guard.get_mut(id) {
            let current_completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
            let new_completed = current_completed + downloaded_chunk;
            control.status.completed_length = new_completed.to_string();

            if worker_id < control.status.worker_progress.len() {
                control.status.worker_progress[worker_id] += downloaded_chunk;
            }

            let now = std::time::Instant::now();
            let elapsed = now.duration_since(control.last_update_time).as_secs_f64();
            if elapsed >= 0.5 {
                let bytes_diff = new_completed - control.last_update_bytes;
                let speed = (bytes_diff as f64 / elapsed) as u64;
                control.status.download_speed = speed.to_string();
                control.last_update_bytes = new_completed;
                control.last_update_time = now;
            }
        }
    }

    /// Spawns the underlying background worker engine for a task.
    ///
    /// Lifecycle stages:
    /// 1. Constructs and launches `DownloadTask`.
    /// 2. Loops over progress channel receiver, updating in-memory status and speed.
    /// 3. On completion:
    ///    - Verifies expected SHA256 integrity hash.
    ///    - Promotes staged `.part` file to final destination.
    ///    - Executes format transcoding if required.
    ///    - Clears macOS quarantine attribute.
    ///    - Emits `pin.onDownloadComplete` event.
    /// 4. Saves session checkpoint.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_task(
        id: String,
        urls: Vec<String>,
        filename: String,
        dir: String,
        threads: usize,
        worker_progress: Vec<u64>,
        headers: Vec<String>,
        expected_hash: Option<String>,
        token: CancellationToken,
        global_limit: Arc<AtomicU64>,
        active_threads: Arc<AtomicU64>,
        global_options: HashMap<String, String>,
        tasks: Arc<RwLock<HashMap<String, TaskControl>>>,
        global_opts_lock: Arc<RwLock<HashMap<String, String>>>,
        tx: broadcast::Sender<String>,
    ) {
        let id_clone = id.clone();
        let dir_clone = dir.clone();
        let filename_clone = filename.clone();

        tokio::spawn(async move {
            let task = DownloadTask {
                urls,
                filename: filename.clone(),
                save_path: dir.clone(),
                threads,
                worker_progress,
                headers,
                global_limit,
                active_threads,
                global_options,
            };

            match task.start(token.clone()).await {
                Ok((
                    total_size,
                    actual_threads,
                    file_type,
                    is_resumable,
                    mut progress_rx,
                    part_filename,
                )) => {
                    {
                        let mut locks = tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            control.status.total_length = total_size.to_string();
                            control.status.file_type = file_type;
                            control.status.is_resumable = Some(is_resumable);

                            if actual_threads != control.status.worker_progress.len() {
                                control.status.worker_progress.resize(actual_threads, 0);
                                control
                                    .options
                                    .insert("split".to_string(), actual_threads.to_string());
                            }
                        }
                    }

                    // Progress channel event loop
                    while let Some((worker_id, bytes_chunk)) = progress_rx.recv().await {
                        Self::update_progress(&id_clone, worker_id, bytes_chunk, &tasks).await;
                    }

                    {
                        let mut locks = tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            if token.is_cancelled() {
                                control.status.status = "paused".to_string();
                                EventNotifier::emit(&tx, "pin.onDownloadPause", &id_clone);
                            } else {
                                let completed =
                                    control.status.completed_length.parse::<u64>().unwrap_or(0);
                                let total = control.status.total_length.parse::<u64>().unwrap_or(0);

                                if total > 0 && completed < total {
                                    control.status.status = "error".to_string();
                                    EventNotifier::emit(&tx, "pin.onDownloadError", &id_clone);
                                } else {
                                    let url = control
                                        .status
                                        .files
                                        .first()
                                        .and_then(|f| f.uris.first().map(|u| u.uri.clone()))
                                        .unwrap_or_default();
                                    let ft = control.status.file_type.clone();
                                    let final_path = format!("{}/{}", dir, filename);
                                    let part_path = format!("{}/{}", dir, part_filename);

                                    drop(locks);

                                    // Verify SHA256 integrity hash if provided
                                    let mut hash_valid = true;
                                    if let Some(expected) = &expected_hash {
                                        hash_valid =
                                            TaskPostProcessor::verify_sha256(&part_path, expected)
                                                .await;
                                    }

                                    if !hash_valid {
                                        let _ = std::fs::remove_file(&part_path);
                                        let mut locks = tasks.write().await;
                                        if let Some(control) = locks.get_mut(&id_clone) {
                                            control.status.status = "error".to_string();
                                            EventNotifier::emit(
                                                &tx,
                                                "pin.onDownloadError",
                                                &id_clone,
                                            );
                                        }
                                        return;
                                    }

                                    // Finalize download promotion and cleanup
                                    TaskPostProcessor::promote_and_cleanup(
                                        &dir,
                                        &filename,
                                        &part_filename,
                                    );
                                    let _ = TaskPostProcessor::handle_conversion(
                                        &final_path,
                                        &url,
                                        ft.as_deref(),
                                    )
                                    .await;
                                    TaskPostProcessor::remove_quarantine(&final_path);

                                    let mut locks = tasks.write().await;
                                    if let Some(control) = locks.get_mut(&id_clone) {
                                        control.status.status = "complete".to_string();
                                        EventNotifier::emit(
                                            &tx,
                                            "pin.onDownloadComplete",
                                            &id_clone,
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Task '{}' failed: {}", id_clone, e);
                    let mut locks = tasks.write().await;
                    if let Some(control) = locks.get_mut(&id_clone) {
                        control.status.status = "error".to_string();
                        crate::engine::DownloadBundle::cleanup_bundle(&dir_clone, &filename_clone);
                        let final_path = format!("{}/{}", dir_clone, filename_clone);
                        let _ = std::fs::remove_file(&final_path);
                    }
                    EventNotifier::emit(&tx, "pin.onDownloadError", &id_clone);
                }
            }

            let tasks_guard = tasks.read().await;
            let global_opts = global_opts_lock.read().await;
            SessionPersistence::save(&tasks_guard, &global_opts);
        });
    }
}
