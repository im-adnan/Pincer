//! BitTorrent session coordinator
//!
//! ### Architectural Overview
//! - **What it does**: Spawns BitTorrent download tasks, registers them in manager memory, and executes continuous background polling loops to stream live progress, seeds, and download/upload speeds.
//! - **How it does**: Calls `session.add_torrent()`, registers the active `ManagedTorrent` handle, initializes `TorrentSnapshot` polling intervals via `tokio::time::interval(500ms)`, updates `TaskStatus` fields, pauses upon completion unless `keep-seeding` is active, and triggers unselected file cleanup immediately and upon completion.
//! - **Where it comes from**: Called by `manager.spawn_torrent_task()` and `manager.unpause_task()`.
//! - **Where it leads to**: Drives the peer-to-peer BitTorrent download pipeline and emits `pin.onDownloadProgress` and `pin.onDownloadComplete` events.

use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use super::notifier::EventNotifier;
use super::persistence::SessionPersistence;
use super::state::TaskControl;
use crate::models::{default_created_at, TaskStatus, TorrentInfo, TorrentInfoInner};
use crate::torrent::{TorrentFileSelector, TorrentStatsTracker, TorrentTaskSpawner};

/// Coordinates BitTorrent task execution and live statistic polling.
pub struct TorrentOrchestrator;

/// Configuration parameters for a new BitTorrent task.
pub struct TorrentTaskContext {
    pub id: String,
    pub torrent_source: AddTorrent<'static>,
    pub dir: String,
    pub options: HashMap<String, String>,
    pub session: Arc<Session>,
    pub tasks: Arc<RwLock<HashMap<String, TaskControl>>>,
    pub torrent_handles: Arc<RwLock<HashMap<String, Arc<ManagedTorrent>>>>,
    pub global_options: Arc<RwLock<HashMap<String, String>>>,
    pub tx: broadcast::Sender<String>,
}

impl TorrentOrchestrator {
    /// Registers a new BitTorrent task and spawns its background session runner.
    pub async fn spawn_task(ctx: TorrentTaskContext) -> Result<String, String> {
        let TorrentTaskContext {
            id,
            torrent_source,
            dir,
            options,
            session,
            tasks,
            torrent_handles,
            global_options,
            tx,
        } = ctx;

        let (initial_name, initial_info_hash, source_url) =
            TorrentTaskSpawner::extract_source_info(&torrent_source);

        let initial_status = TaskStatus {
            gid: id.clone(),
            status: "active".to_string(),
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            upload_speed: None,
            worker_progress: vec![],
            file_type: Some("torrent".to_string()),
            is_resumable: Some(true),
            files: vec![],
            dir: dir.clone(),
            bittorrent: Some(TorrentInfo {
                announce_list: vec![],
                comment: None,
                creation_date: None,
                mode: "single".to_string(),
                info: TorrentInfoInner {
                    name: initial_name.clone(),
                },
            }),
            info_hash: initial_info_hash,
            num_seeders: Some(0),
            url: source_url,
        };

        let token = CancellationToken::new();

        {
            let mut tasks_guard = tasks.write().await;
            let mut opts = options.clone();
            opts.insert("dir".to_string(), dir.clone());
            opts.insert("out".to_string(), initial_name.clone());
            tasks_guard.insert(
                id.clone(),
                TaskControl::new(
                    initial_status,
                    token.clone(),
                    opts,
                    None,
                    default_created_at(),
                ),
            );
        }

        EventNotifier::emit(&tx, "pin.onDownloadStart", &id);

        let only_files = if let Some(files_str) = options.get("select-files") {
            let parsed: Vec<usize> = files_str
                .split(',')
                .filter_map(|s| s.parse::<usize>().ok())
                .collect();
            if parsed.is_empty() {
                None
            } else {
                Some(parsed)
            }
        } else {
            None
        };

        let id_clone = id.clone();
        let session_clone = session.clone();
        let dir_clone = dir.clone();
        let tasks_clone = tasks.clone();
        let handles_clone = torrent_handles.clone();
        let global_opts_clone = global_options.clone();
        let tx_clone = tx.clone();
        let token_clone = token.clone();

        tokio::spawn(async move {
            let (final_dir, resolved_name) = TorrentTaskSpawner::prepare_output_directory(
                &session_clone,
                &torrent_source,
                &initial_name,
                &dir_clone,
            )
            .await;

            // Update task status with the exact torrent folder directory
            {
                let mut tasks_guard = tasks_clone.write().await;
                if let Some(control) = tasks_guard.get_mut(&id_clone) {
                    control.status.dir = final_dir.clone();
                    if let Some(bt) = control.status.bittorrent.as_mut() {
                        bt.info.name = resolved_name.clone();
                    }
                }
            }

            let opts = AddTorrentOptions {
                overwrite: true,
                output_folder: Some(final_dir),
                only_files,
                ..Default::default()
            };

            match session_clone.add_torrent(torrent_source, Some(opts)).await {
                Ok(res) => {
                    if let Some(handle) = res.into_handle() {
                        {
                            let mut handles = handles_clone.write().await;
                            handles.insert(id_clone.clone(), handle.clone());
                        }

                        println!(
                            "[PINCER] Torrent active for id={}, name={:?}",
                            id_clone, resolved_name
                        );

                        // Start continuous progress polling loop
                        Self::run_stats_loop(
                            id_clone,
                            handle,
                            token_clone,
                            session_clone,
                            tasks_clone,
                            global_opts_clone,
                            tx_clone,
                        )
                        .await;
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[PINCER ERR] Failed to add torrent for id={}: {:?}",
                        id_clone, e
                    );
                    let mut tasks_guard = tasks_clone.write().await;
                    if let Some(control) = tasks_guard.get_mut(&id_clone) {
                        control.status.status = "error".to_string();
                    }
                    EventNotifier::emit(&tx_clone, "pin.onDownloadError", &id_clone);
                }
            }
        });

        {
            let tasks_guard = tasks.read().await;
            let global_opts = global_options.read().await;
            SessionPersistence::save(&tasks_guard, &global_opts);
        }

        Ok(id)
    }

    /// Continuous background polling loop (every 500ms) updating live BitTorrent stats.
    pub async fn run_stats_loop(
        id: String,
        handle: Arc<ManagedTorrent>,
        token: CancellationToken,
        session: Arc<Session>,
        tasks: Arc<RwLock<HashMap<String, TaskControl>>>,
        global_options: Arc<RwLock<HashMap<String, String>>>,
        tx: broadcast::Sender<String>,
    ) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        let mut unselected_cleaned = false;

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {
                    let dir = {
                        let tasks_guard = tasks.read().await;
                        tasks_guard.get(&id).map(|c| c.status.dir.clone()).unwrap_or_default()
                    };

                    let snap = TorrentStatsTracker::snapshot(&handle, &dir);
                    let mut task_finished = false;
                    let mut keep_seeding = false;

                    {
                        let mut tasks_guard = tasks.write().await;
                        if let Some(control) = tasks_guard.get_mut(&id) {
                            control.status.total_length = snap.total_bytes.to_string();
                            control.status.completed_length = snap.progress_bytes.to_string();
                            control.status.download_speed = snap.download_speed.to_string();
                            control.status.upload_speed = Some(snap.upload_speed.to_string());
                            control.status.num_seeders = Some(snap.peer_count);
                            control.status.bittorrent = snap.bittorrent;
                            if !snap.files.is_empty() {
                                control.status.files = snap.files;
                            }
                            if snap.finished {
                                control.status.status = "complete".to_string();
                                task_finished = true;
                                keep_seeding = control.options.get("keep-seeding").map(|s| s == "true").unwrap_or(false);
                            }
                        }
                    }

                    // Clean up unselected placeholder files and empty folders as soon as metadata is active
                    if !unselected_cleaned {
                        let files_opt = {
                            let tasks_guard = tasks.read().await;
                            tasks_guard.get(&id).and_then(|c| c.options.get("select-files").cloned())
                        };

                        if let Some(files_str) = files_opt {
                            if let Some(meta) = &*handle.metadata.load() {
                                if let Some(files) = &meta.info.files {
                                    TorrentFileSelector::cleanup_unselected_files(&dir, &files_str, files);
                                    unselected_cleaned = true;
                                }
                            }
                        }
                    }

                    if task_finished && !keep_seeding {
                        let _ = session.pause(&handle).await;
                        let mut tasks_guard = tasks.write().await;
                        if let Some(control) = tasks_guard.get_mut(&id) {
                            control.status.status = "paused".to_string();
                        }
                    }

                    {
                        let tasks_guard = tasks.read().await;
                        let global_opts = global_options.read().await;
                        SessionPersistence::save(&tasks_guard, &global_opts);
                    }

                    EventNotifier::emit(&tx, "pin.onDownloadProgress", &id);

                    if task_finished {
                        let files_opt = {
                            let tasks_guard = tasks.read().await;
                            tasks_guard.get(&id).and_then(|c| c.options.get("select-files").cloned())
                        };

                        if let Some(files_str) = files_opt {
                            if let Some(meta) = &*handle.metadata.load() {
                                if let Some(files) = &meta.info.files {
                                    TorrentFileSelector::cleanup_unselected_files(&dir, &files_str, files);
                                }
                            }
                        }

                        EventNotifier::emit(&tx, "pin.onDownloadComplete", &id);
                        break;
                    }
                }
            }
        }
    }
}
