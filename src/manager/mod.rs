//! DownloadManager struct definition, constructor & facade
//!
//! ### Architectural Overview
//! - **What it does**: Acts as the central singleton facade for Pincer, coordinating in-memory task registries, queue scheduling, options management, BitTorrent lifecycle, and RPC broadcast channels.
//! - **How it does**: Maintains shared thread-safe state via `Arc<RwLock<HashMap<String, TaskControl>>>`, `Arc<AtomicU64>` rate limiters, Tokio broadcast channels, and delegates operations to specialized domain submodules.
//! - **Where it comes from**: Instantiated in `src/main.rs` and shared across CLI dispatchers, HTTP/WebSocket servers, and background workers.
//! - **Where it leads to**: Coordinates task execution across `engine::DownloadTask`, `torrent::TorrentSessionManager`, and broadcasts JSON-RPC event notifications to connected clients.

pub mod execution;
pub mod lifecycle;
pub mod notifier;
pub mod options;
pub mod persistence;
pub mod query;
pub mod removal;
pub mod scheduler;
pub mod session_loader;
pub mod spawner;
pub mod speed;
pub mod state;
pub mod task_runner;
pub mod torrent_orchestrator;
pub mod unique_name;

use librqbit::{AddTorrent, ManagedTorrent, Session};
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::{broadcast, OnceCell, RwLock};

// Re-export manager domain handlers
pub use execution::TaskPostProcessor;
pub use lifecycle::TaskLifecycleManager;
pub use notifier::EventNotifier;
pub use options::OptionsManager;
pub use persistence::SessionPersistence;
pub use query::TaskQueryManager;
pub use removal::TaskRemovalManager;
pub use scheduler::TaskScheduler;
pub use session_loader::SessionLoader;
pub use spawner::TaskSpawner;
pub use speed::SpeedCalculator;
pub use state::TaskControl;
pub use task_runner::TaskRunner;
pub use torrent_orchestrator::TorrentOrchestrator;
pub use unique_name::UniqueNameGenerator;

use crate::converter::FormatTranscoder;
use crate::models::{GlobalStat, ResolveResponse, TaskStatus};
use crate::resolver::{TorrentResolver, UniversalResolver};
use crate::torrent::TorrentSessionManager;

/// The central orchestrator facade for the Pincer engine.
///
/// Thread safety: All internal state is guarded via `Arc<RwLock<...>>` or atomics (`Arc<AtomicU64>`),
/// allowing concurrent calls across multiple WebSocket connections and background worker tasks.
pub struct DownloadManager {
    /// In-memory registry of all download tasks indexed by unique GID.
    pub tasks: Arc<RwLock<HashMap<String, TaskControl>>>,
    /// Global engine configuration options key-value store.
    pub global_options: Arc<RwLock<HashMap<String, String>>>,
    /// Tokio broadcast channel for real-time JSON-RPC event notifications.
    pub tx: broadcast::Sender<String>,
    /// Global bandwidth limit in bytes/second (0 = unlimited).
    pub current_limit: Arc<AtomicU64>,
    /// Peak observed bandwidth rate for adaptive speed-mode scaling.
    pub max_seen_speed: Arc<AtomicU64>,
    /// Counter of currently active worker connection threads.
    pub active_threads: Arc<AtomicU64>,
    /// Default connection split count for newly added downloads.
    pub default_split: Arc<AtomicU64>,
    /// Lazy-initialized BitTorrent peer session singleton.
    pub torrent_session: OnceCell<Arc<Session>>,
    /// Registry of live `ManagedTorrent` session handles.
    pub torrent_handles: Arc<RwLock<HashMap<String, Arc<ManagedTorrent>>>>,
}

impl DownloadManager {
    /// Creates a new `DownloadManager` instance and initializes its notification broadcast channel.
    pub fn new() -> (Arc<Self>, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(100);
        let manager = Arc::new(Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            global_options: Arc::new(RwLock::new(HashMap::new())),
            tx,
            current_limit: Arc::new(AtomicU64::new(0)),
            max_seen_speed: Arc::new(AtomicU64::new(10 * 1024 * 1024)),
            active_threads: Arc::new(AtomicU64::new(0)),
            default_split: Arc::new(AtomicU64::new(1)),
            torrent_session: OnceCell::new(),
            torrent_handles: Arc::new(RwLock::new(HashMap::new())),
        });
        (manager, rx)
    }

    /// Returns the semantic package version compiled into the binary.
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Returns the shared `librqbit::Session`, initializing it lazily on the first BitTorrent request.
    pub async fn get_torrent_session(&self) -> Result<Arc<Session>, String> {
        self.torrent_session
            .get_or_try_init(TorrentSessionManager::create_session)
            .await
            .cloned()
    }

    /// Subscribes a new WebSocket client to the JSON-RPC event broadcast channel.
    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    /// Generates a non-colliding filename if another task already uses the same name.
    pub async fn generate_unique_filename(
        &self,
        filename: &str,
        excluding_gid: Option<&str>,
    ) -> String {
        let tasks_guard = self.tasks.read().await;
        UniqueNameGenerator::generate_unique(filename, excluding_gid, &tasks_guard)
    }

    /// Registers a new direct download task and triggers the queue scheduler.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_task(
        self: &Arc<Self>,
        id: String,
        urls: Vec<String>,
        filename: String,
        dir: String,
        threads: usize,
        resume_offset: u64,
        headers: Vec<String>,
        expected_hash: Option<String>,
    ) {
        TaskSpawner::register_task(
            id,
            urls,
            &filename,
            dir,
            threads,
            resume_offset,
            headers,
            expected_hash,
            &self.tasks,
        )
        .await;

        self.save_session().await;
        self.schedule_tasks().await;
    }

    /// Registers and launches a new BitTorrent download task.
    pub async fn spawn_torrent_task(
        self: &Arc<Self>,
        id: String,
        torrent_source: AddTorrent<'static>,
        dir: String,
        options: HashMap<String, String>,
    ) -> Result<String, String> {
        let session = self.get_torrent_session().await?;
        TorrentOrchestrator::spawn_task(crate::manager::torrent_orchestrator::TorrentTaskContext {
            id,
            torrent_source,
            dir,
            options,
            session,
            tasks: self.tasks.clone(),
            torrent_handles: self.torrent_handles.clone(),
            global_options: self.global_options.clone(),
            tx: self.tx.clone(),
        })
        .await
    }

    /// Evaluates waiting tasks in the queue and starts eligible downloads respecting concurrency limits.
    pub fn schedule_tasks<'a>(
        self: &'a Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let runnable =
                TaskScheduler::select_runnable_tasks(&self.tasks, &self.global_options).await;
            for gid in runnable {
                self.execute_task(gid).await;
            }
        })
    }

    /// Spawns background worker execution for an individual task GID.
    pub async fn execute_task(self: &Arc<Self>, id: String) {
        let (urls, filename, dir, threads, worker_progress, headers, expected_hash, token) = {
            let mut locks = self.tasks.write().await;
            if let Some(control) = locks.get_mut(&id) {
                control.status.status = "active".to_string();
                let token = control.token.clone();
                let urls = control
                    .status
                    .files
                    .first()
                    .map(|f| f.uris.iter().map(|u| u.uri.clone()).collect())
                    .unwrap_or_default();
                let filename = std::path::Path::new(&control.status.files[0].path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let dir = control.status.dir.clone();
                let threads = control
                    .options
                    .get("split")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);
                let worker_progress = control.status.worker_progress.clone();
                let headers = control
                    .options
                    .get("header")
                    .map(|h| {
                        h.split('\n')
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let expected_hash = control.expected_hash.clone();
                (
                    urls,
                    filename,
                    dir,
                    threads,
                    worker_progress,
                    headers,
                    expected_hash,
                    token,
                )
            } else {
                return;
            }
        };

        EventNotifier::emit(&self.tx, "pin.onDownloadStart", &id);
        let global_opts = self.global_options.read().await.clone();

        TaskRunner::run_task(
            id,
            urls,
            filename,
            dir,
            threads,
            worker_progress,
            headers,
            expected_hash,
            token,
            self.current_limit.clone(),
            self.active_threads.clone(),
            global_opts,
            self.tasks.clone(),
            self.global_options.clone(),
            self.tx.clone(),
        )
        .await;
    }

    /// Pauses an active or waiting task by GID.
    pub async fn pause_task(self: &Arc<Self>, id: &str) -> bool {
        let session = self.get_torrent_session().await.ok();
        let res = TaskLifecycleManager::pause_task(
            id,
            &self.tasks,
            &self.torrent_handles,
            session.as_ref(),
            &self.tx,
        )
        .await;
        if res {
            self.save_session().await;
            self.schedule_tasks().await;
        }
        res
    }

    /// Pauses all active and waiting tasks across the engine.
    pub async fn pause_all_tasks(self: &Arc<Self>) {
        TaskLifecycleManager::pause_all(&self.tasks, &self.tx).await;
        self.save_session().await;
        self.schedule_tasks().await;
    }

    /// Resumes a paused or stopped task.
    pub async fn unpause_task(self: &Arc<Self>, id: &str) -> bool {
        let is_torrent = {
            let tasks_guard = self.tasks.read().await;
            tasks_guard.get(id).and_then(|c| c.status.file_type.clone())
                == Some("torrent".to_string())
        };

        if is_torrent {
            if let Ok(session) = self.get_torrent_session().await {
                if let Some((handle, token)) = TaskLifecycleManager::unpause_torrent(
                    id,
                    &self.tasks,
                    &self.torrent_handles,
                    &session,
                )
                .await
                {
                    let session_clone = session.clone();
                    let tasks_clone = self.tasks.clone();
                    let global_opts_clone = self.global_options.clone();
                    let tx_clone = self.tx.clone();
                    let id_str = id.to_string();

                    tokio::spawn(async move {
                        TorrentOrchestrator::run_stats_loop(
                            id_str,
                            handle,
                            token,
                            session_clone,
                            tasks_clone,
                            global_opts_clone,
                            tx_clone,
                        )
                        .await;
                    });
                    EventNotifier::emit(&self.tx, "pin.onDownloadStart", id);
                    self.save_session().await;
                    return true;
                }
            }
            return false;
        }

        let (url, filename, dir, mut resume_offset, threads, headers, is_resumable) = {
            let tasks_guard = self.tasks.read().await;
            if let Some(control) = tasks_guard.get(id) {
                if (control.status.status == "active" || control.status.status == "converting")
                    && !control.token.is_cancelled()
                {
                    return false;
                }
                if control.status.status == "complete" {
                    return false;
                }
                let url = control.status.files[0].uris[0].uri.clone();
                let filename = std::path::Path::new(&control.status.files[0].path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let dir = control.status.dir.clone();
                let resume_offset = control.status.completed_length.parse::<u64>().unwrap_or(0);
                let threads = control
                    .options
                    .get("split")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1);
                let headers: Vec<String> = control
                    .options
                    .get("header")
                    .map(|h| {
                        h.split('\n')
                            .filter(|s| !s.trim().is_empty())
                            .map(|s| s.trim().to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                let is_resumable = control.status.is_resumable;
                (
                    url,
                    filename,
                    dir,
                    resume_offset,
                    threads,
                    headers,
                    is_resumable,
                )
            } else {
                return false;
            }
        };

        if is_resumable == Some(false) {
            resume_offset = 0;
            let path_str = format!("{}/{}", dir, filename);
            let part_path_str = format!("{}/.{}.pincer", dir, filename);
            let _ = std::fs::remove_file(path_str);
            crate::engine::DownloadBundle::cleanup_bundle(&dir, &filename);
            let _ = std::fs::remove_file(part_path_str);
        }

        self.spawn_task(
            id.to_string(),
            vec![url],
            filename,
            dir,
            threads,
            resume_offset,
            headers,
            None,
        )
        .await;
        self.save_session().await;
        true
    }

    /// Resumes all paused and stopped tasks across the engine.
    pub async fn unpause_all_tasks(self: &Arc<Self>) {
        let gids: Vec<String> = {
            let tasks_guard = self.tasks.read().await;
            tasks_guard
                .iter()
                .filter(|(_, c)| {
                    let is_completed_torrent = c.status.file_type.as_deref() == Some("torrent")
                        && {
                            let completed = c.status.completed_length.parse::<u64>().unwrap_or(0);
                            let total = c.status.total_length.parse::<u64>().unwrap_or(0);
                            total > 0 && completed >= total
                        };
                    if is_completed_torrent {
                        false
                    } else {
                        c.status.status == "paused"
                            || c.status.status == "waiting"
                            || c.status.status == "error"
                            || c.status.status == "removed"
                    }
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for gid in gids {
            self.unpause_task(&gid).await;
        }
    }

    /// Replaces or updates the mirror URIs for a specific task.
    pub async fn change_uri(
        &self,
        id: &str,
        file_index: usize,
        del_uris: Vec<String>,
        add_uris: Vec<String>,
    ) -> Result<(usize, usize), String> {
        let mut tasks_guard = self.tasks.write().await;
        if let Some(control) = tasks_guard.get_mut(id) {
            if control.status.status == "active" || control.status.status == "converting" {
                return Err("Cannot change URI of an active task. Pause it first.".to_string());
            }
            if file_index > 0 || control.status.files.is_empty() {
                return Err("Invalid file index.".to_string());
            }
            let mut deleted = 0;
            let mut added = 0;
            let current_uri = control.status.files[0].uris[0].uri.clone();
            let mut new_uri = current_uri.clone();

            if del_uris.contains(&current_uri) {
                deleted = 1;
                new_uri = String::new();
            }
            if let Some(first_add) = add_uris.first() {
                new_uri = first_add.clone();
                added = 1;
            }
            if new_uri.is_empty() {
                return Err(
                    "Cannot remove the only URI without providing a replacement.".to_string(),
                );
            }
            control.status.files[0].uris[0].uri = new_uri;
            Ok((deleted, added))
        } else {
            Err("GID not found.".to_string())
        }
    }

    /// Removes a task from the active registry without deleting files on disk.
    pub async fn remove_task(self: &Arc<Self>, id: &str) -> bool {
        let session = self.get_torrent_session().await.ok();
        let res = TaskRemovalManager::remove_task(
            id,
            &self.tasks,
            &self.torrent_handles,
            session.as_ref(),
        )
        .await;
        if res {
            self.save_session().await;
            self.schedule_tasks().await;
        }
        res
    }

    /// Removes a task and moves all associated download files to Trash in the background.
    pub async fn remove_task_and_file(self: &Arc<Self>, id: &str) -> bool {
        let session = self.get_torrent_session().await.ok();
        let res = TaskRemovalManager::remove_task_and_file(
            id,
            &self.tasks,
            &self.torrent_handles,
            session.as_ref(),
        )
        .await;
        if res {
            self.save_session().await;
            self.schedule_tasks().await;
        }
        res
    }

    /// Force removes a task, deleting files immediately if it is a torrent task.
    pub async fn force_remove_task(self: &Arc<Self>, id: &str) -> bool {
        let is_torrent = {
            let tasks_guard = self.tasks.read().await;
            tasks_guard.get(id).and_then(|c| c.status.file_type.clone())
                == Some("torrent".to_string())
        };

        if is_torrent {
            return self.remove_task_and_file(id).await;
        }
        self.remove_task(id).await
    }

    /// Queries status for an individual task GID.
    pub async fn get_task(&self, id: &str) -> Option<TaskStatus> {
        TaskQueryManager::get_task(id, &self.tasks).await
    }

    /// Queries all currently active or converting tasks.
    pub async fn get_active_tasks(&self) -> Vec<TaskStatus> {
        TaskQueryManager::get_active(&self.tasks).await
    }

    /// Queries waiting and paused tasks.
    pub async fn get_waiting_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        TaskQueryManager::get_waiting(&self.tasks).await
    }

    /// Queries stopped, completed, or failed tasks.
    pub async fn get_stopped_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        TaskQueryManager::get_stopped(&self.tasks).await
    }

    /// Computes aggregate global bandwidth metrics and task counters.
    pub async fn get_global_stat(&self) -> GlobalStat {
        TaskQueryManager::get_global_stat(&self.tasks, &self.global_options, &self.max_seen_speed)
            .await
    }

    /// Updates global configuration options and bandwidth speed modes.
    pub async fn change_global_option(self: &Arc<Self>, options: HashMap<String, String>) {
        OptionsManager::apply_global_options(
            options,
            &self.global_options,
            &self.current_limit,
            &self.max_seen_speed,
            &self.default_split,
        )
        .await;
        self.save_session().await;
        self.schedule_tasks().await;
    }

    /// Returns a copy of the current global configuration options map.
    pub async fn get_global_option(&self) -> HashMap<String, String> {
        self.global_options.read().await.clone()
    }

    /// Updates configuration options for a specific task GID.
    pub async fn change_option(
        self: &Arc<Self>,
        id: &str,
        options: HashMap<String, String>,
    ) -> bool {
        let (res, change_seeding_to) =
            OptionsManager::apply_task_options(id, options, &self.tasks).await;

        if let Some(should_seed) = change_seeding_to {
            let handle = {
                let handles = self.torrent_handles.read().await;
                handles.get(id).cloned()
            };
            if let (Some(handle), Ok(session)) = (handle, self.get_torrent_session().await) {
                if should_seed {
                    if session.unpause(&handle).await.is_ok() {
                        let token = tokio_util::sync::CancellationToken::new();
                        let mut tasks_guard = self.tasks.write().await;
                        if let Some(control) = tasks_guard.get_mut(id) {
                            control.status.status = "active".to_string();
                            control.token = token.clone();
                        }
                        let session_clone = session.clone();
                        let tasks_clone = self.tasks.clone();
                        let global_opts_clone = self.global_options.clone();
                        let tx_clone = self.tx.clone();
                        let id_str = id.to_string();
                        tokio::spawn(async move {
                            TorrentOrchestrator::run_stats_loop(
                                id_str,
                                handle,
                                token,
                                session_clone,
                                tasks_clone,
                                global_opts_clone,
                                tx_clone,
                            )
                            .await;
                        });
                    }
                } else {
                    let _ = session.pause(&handle).await;
                    let mut tasks_guard = self.tasks.write().await;
                    if let Some(control) = tasks_guard.get_mut(id) {
                        control.token.cancel();
                        control.status.status = "paused".to_string();
                    }
                }
            }
        }

        if res {
            self.save_session().await;
        }
        res
    }

    /// Queries custom options map for a specific task GID.
    pub async fn get_option(&self, id: &str) -> Option<HashMap<String, String>> {
        let tasks_guard = self.tasks.read().await;
        tasks_guard.get(id).map(|c| c.options.clone())
    }

    /// Purges all stopped, completed, and failed tasks from memory.
    pub async fn purge_download_result(&self) {
        let mut tasks_guard = self.tasks.write().await;
        tasks_guard.retain(|_, control| {
            control.status.status != "complete"
                && control.status.status != "error"
                && control.status.status != "removed"
        });
    }

    /// Removes an individual stopped or completed task result from memory.
    pub async fn remove_download_result(&self, id: &str) -> bool {
        let mut tasks_guard = self.tasks.write().await;
        if let Some(control) = tasks_guard.get(id) {
            if control.status.status == "complete"
                || control.status.status == "error"
                || control.status.status == "removed"
            {
                tasks_guard.remove(id);
                return true;
            }
        }
        false
    }

    /// Probes remote URL or Magnet metadata.
    pub async fn resolve_url(&self, url: String) -> Result<ResolveResponse, String> {
        let session = self.get_torrent_session().await?;
        let global_opts = self.global_options.read().await.clone();
        UniversalResolver::resolve_url(url, &session, &global_opts).await
    }

    /// Probes Base64-encoded `.torrent` metadata.
    pub async fn resolve_torrent_base64(
        &self,
        base64_str: String,
    ) -> Result<ResolveResponse, String> {
        let session = self.get_torrent_session().await?;
        TorrentResolver::resolve_base64(&session, &base64_str).await
    }

    /// Performs post-download format transcoding.
    pub async fn perform_format_conversion(
        &self,
        file_path_str: &str,
        url: &str,
        file_type: Option<&str>,
    ) -> Result<(), String> {
        FormatTranscoder::convert_format(file_path_str, url, file_type).await
    }

    /// Serializes active and completed tasks to `~/.pincer/pincer.session`.
    pub async fn save_session(&self) {
        let tasks_guard = self.tasks.read().await;
        let global_opts = self.global_options.read().await;
        SessionPersistence::save(&tasks_guard, &global_opts);
    }

    /// Restores saved tasks and global settings from `pincer.session`.
    pub async fn load_session(self: &Arc<Self>) {
        SessionLoader::restore_session(
            &self.tasks,
            &self.global_options,
            &self.default_split,
            &self.current_limit,
        )
        .await;
    }
}
