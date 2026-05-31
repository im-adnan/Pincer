use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::{
    FileData, FileUri, GlobalStat, NotificationParam, RPCNotification, TaskStatus,
};

/// Internal control structure for managing a single download task's state.
/// Holds the cancellation token, specific task options, and live speed calculation tracking data.
struct TaskControl {
    status: TaskStatus,
    token: CancellationToken,
    options: HashMap<String, String>,
    last_update_bytes: u64,
    last_update_time: std::time::Instant,
}

/// The central orchestrator of the Pincer engine.
/// Holds the global state, all active/paused tasks, global bandwidth limits, and handles
/// broadcasting status updates to all connected WebSocket clients via the `tx` channel.
pub struct DownloadManager {
    tasks: RwLock<HashMap<String, TaskControl>>,
    global_options: RwLock<HashMap<String, String>>,
    tx: broadcast::Sender<String>,
    pub current_limit: Arc<AtomicU64>,
    pub max_seen_speed: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
    pub default_split: Arc<AtomicU64>,
}

impl DownloadManager {
    pub fn new() -> (Arc<Self>, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(100);
        let manager = Arc::new(Self {
            tasks: RwLock::new(HashMap::new()),
            global_options: RwLock::new(HashMap::new()),
            tx,
            current_limit: Arc::new(AtomicU64::new(0)),
            max_seen_speed: Arc::new(AtomicU64::new(10 * 1024 * 1024)), // Default 10MB/s for initial half half
            active_threads: Arc::new(AtomicU64::new(0)),
            default_split: Arc::new(AtomicU64::new(1)),
        });
        (manager, rx)
    }

    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    fn get_session_path(&self) -> std::path::PathBuf {
        let home = std::env::var("HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = home.join(".pincer");
        let _ = std::fs::create_dir_all(&dir);
        dir.join("pincer.session")
    }

    /// Serializes all current tasks and global options to `~/.pincer/pincer.session`.
    /// Active tasks are marked as "paused" in the session file so they do not auto-resume on restart.
    pub async fn save_session(&self) {
        let tasks = self.tasks.read().await;
        let global_options = self.global_options.read().await;

        let mut session_tasks = Vec::new();
        for (id, control) in tasks.iter() {
            let status = &control.status;
            // If the task was active or waiting, save it as paused so it does not auto-resume on startup
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
                    std::path::Path::new(&f.path)
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

            session_tasks.push(crate::models::SessionTask {
                id: id.clone(),
                url: first_uri,
                filename,
                save_path: status.dir.clone(),
                threads: control
                    .options
                    .get("split")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(4),
                headers,
                status: saved_status,
                total_length: status.total_length.parse::<u64>().unwrap_or(0),
                completed_length: status.completed_length.parse::<u64>().unwrap_or(0),
            });
        }

        let session_data = crate::models::SessionData {
            tasks: session_tasks,
            global_options: global_options.clone(),
        };

        if let Ok(json_str) = serde_json::to_string_pretty(&session_data) {
            let session_path = self.get_session_path();
            if let Err(e) = std::fs::write(&session_path, json_str) {
                eprintln!("Failed to write session file to {:?}: {}", session_path, e);
            } else {
                println!("Session successfully saved to {:?}", session_path);
            }
        }
    }

    /// Deserializes and loads tasks from the `pincer.session` file on startup.
    /// Restores global options, bandwidth limits, and populates the task list.
    pub async fn load_session(self: &Arc<Self>) {
        let session_path = self.get_session_path();
        if !session_path.exists() {
            println!("No session file found at {:?}", session_path);
            return;
        }

        println!("Loading session from {:?}", session_path);
        if let Ok(json_str) = std::fs::read_to_string(&session_path) {
            if let Ok(session_data) = serde_json::from_str::<crate::models::SessionData>(&json_str)
            {
                // Restore global options
                {
                    let mut global_opts = self.global_options.write().await;
                    *global_opts = session_data.global_options;
                    if let Some(split_str) = global_opts.get("default-split") {
                        if let Ok(split) = split_str.parse::<u64>() {
                            self.default_split.store(split, Ordering::Relaxed);
                        }
                    }
                    if let Some(limit_str) = global_opts.get("max-overall-download-limit") {
                        if let Ok(limit) = limit_str.parse::<u64>() {
                            self.current_limit.store(limit, Ordering::Relaxed);
                        }
                    }
                }

                // Restore tasks
                for task in &session_data.tasks {
                    let mut opts = HashMap::new();
                    opts.insert("dir".to_string(), task.save_path.clone());
                    opts.insert("out".to_string(), task.filename.clone());
                    opts.insert("split".to_string(), task.threads.to_string());
                    if !task.headers.is_empty() {
                        opts.insert("header".to_string(), task.headers.join("\n"));
                    }

                    let initial_status = TaskStatus {
                        gid: task.id.clone(),
                        status: task.status.clone(),
                        total_length: task.total_length.to_string(),
                        completed_length: task.completed_length.to_string(),
                        download_speed: "0".to_string(),
                        worker_progress: vec![0; task.threads],
                        file_type: None,
                        is_resumable: Some(true),
                        files: vec![FileData {
                            path: format!("{}/{}", task.save_path, task.filename),
                            uris: vec![FileUri {
                                uri: task.url.clone(),
                            }],
                        }],
                        dir: task.save_path.clone(),
                    };

                    {
                        let mut tasks = self.tasks.write().await;
                        tasks.insert(
                            task.id.clone(),
                            TaskControl {
                                status: initial_status,
                                token: CancellationToken::new(),
                                options: opts,
                                last_update_bytes: 0,
                                last_update_time: std::time::Instant::now(),
                            },
                        );
                    }
                }
                println!(
                    "Session successfully loaded. Restored {} tasks.",
                    session_data.tasks.len()
                );
            } else {
                eprintln!("Failed to parse session file at {:?}", session_path);
            }
        }
    }

    pub async fn generate_unique_filename(
        &self,
        filename: &str,
        excluding_gid: Option<&str>,
    ) -> String {
        let tasks = self.tasks.read().await;
        let mut unique_name = filename.to_string();
        let mut counter = 1;

        let path = std::path::Path::new(filename);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(filename);
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        while tasks.values().any(|c| {
            if let Some(egid) = excluding_gid {
                if c.status.gid == egid {
                    return false;
                }
            }
            if let Some(file) = c.status.files.first() {
                let existing_filename = std::path::Path::new(&file.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if existing_filename == unique_name {
                    return true;
                }
            }
            false
        }) {
            if extension.is_empty() {
                unique_name = format!("{}_{}", stem, counter);
            } else {
                unique_name = format!("{}_{}.{}", stem, counter, extension);
            }
            counter += 1;
        }
        unique_name
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub async fn _add_task(
        &self,
        id: String,
        status: TaskStatus,
        token: CancellationToken,
        options: HashMap<String, String>,
    ) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(
            id.clone(),
            TaskControl {
                status,
                token,
                options,
                last_update_bytes: 0,
                last_update_time: std::time::Instant::now(),
            },
        );
        // Dispatch start event
        let _ = self
            .tx
            .send(self.build_notification("pin.onDownloadStart", &id));
    }

    pub async fn update_task_progress(&self, id: &str, worker_id: usize, downloaded_chunk: u64) {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
            let current_completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
            let new_completed = current_completed + downloaded_chunk;
            control.status.completed_length = new_completed.to_string();

            // Update per-worker progress
            if worker_id < control.status.worker_progress.len() {
                control.status.worker_progress[worker_id] += downloaded_chunk;
            }

            // Calculate speed every ~0.5s to 1s
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

    /// Asynchronously spawns a new download task.
    /// 1. Resolves a unique filename to prevent overwriting.
    /// 2. Initializes the `TaskStatus` and broadcasts `pin.onDownloadStart`.
    /// 3. Spawns the `DownloadTask` engine in the background and tracks its chunked progress.
    #[allow(clippy::too_many_arguments)]
    pub async fn spawn_task(
        self: &Arc<Self>,
        id: String,
        url: String,
        mut filename: String,
        dir: String,
        threads: usize,
        resume_offset: u64,
        headers: Vec<String>,
    ) {
        filename = self.generate_unique_filename(&filename, Some(&id)).await;
        let token = CancellationToken::new();

        let completed_length = if resume_offset > 0 {
            resume_offset.to_string()
        } else {
            "0".to_string()
        };

        // 1. Register the task initially as "active"
        // (or update existing one if it's a resume)
        let initial_status = TaskStatus {
            gid: id.clone(),
            status: "active".to_string(),
            total_length: "0".to_string(), // Will be updated by task.start
            completed_length,
            download_speed: "0".to_string(),
            worker_progress: vec![0; threads],
            file_type: None,
            is_resumable: None,
            files: vec![FileData {
                path: format!("{}/{}", dir, filename),
                uris: vec![FileUri { uri: url.clone() }],
            }],
            dir: dir.clone(),
        };

        {
            let mut tasks = self.tasks.write().await;
            let mut opts = HashMap::new();
            opts.insert("dir".to_string(), dir.clone());
            opts.insert("out".to_string(), filename.clone());
            opts.insert("split".to_string(), threads.to_string());
            if !headers.is_empty() {
                opts.insert("header".to_string(), headers.join("\n"));
            }

            tasks.insert(
                id.clone(),
                TaskControl {
                    status: initial_status,
                    token: token.clone(),
                    options: opts,
                    last_update_bytes: 0,
                    last_update_time: std::time::Instant::now(),
                },
            );
        }

        self.save_session().await;

        let _ = self
            .tx
            .send(self.build_notification("pin.onDownloadStart", &id));

        // 2. Spawn the background rust task
        let manager_clone = self.clone();
        let id_clone = id.clone();
        tokio::spawn(async move {
            let task = crate::task::DownloadTask {
                url,
                filename,
                save_path: dir,
                threads,
                resume_offset,
                headers: headers.clone(),
                global_limit: manager_clone.current_limit.clone(),
                active_threads: manager_clone.active_threads.clone(),
            };

            match task.start(token.clone()).await {
                Ok((total_size, _actual_threads, file_type, is_resumable, mut progress_rx)) => {
                    {
                        let mut locks = manager_clone.tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            control.status.total_length = total_size.to_string();
                            control.status.file_type = file_type;
                            control.status.is_resumable = Some(is_resumable);
                        }
                    }

                    while let Some((worker_id, bytes_chunk)) = progress_rx.recv().await {
                        manager_clone
                            .update_task_progress(&id_clone, worker_id, bytes_chunk)
                            .await;
                    }

                    // Check if it was cancelled or finished
                    {
                        let mut locks = manager_clone.tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            if token.is_cancelled() {
                                control.status.status = "paused".to_string();
                                let _ = manager_clone.tx.send(
                                    manager_clone
                                        .build_notification("pin.onDownloadPause", &id_clone),
                                );
                            } else {
                                // Extract file path, URL and file type for conversion
                                let file_path = control
                                    .status
                                    .files
                                    .first()
                                    .map(|f| f.path.clone())
                                    .unwrap_or_default();
                                let url = control
                                    .status
                                    .files
                                    .first()
                                    .and_then(|f| f.uris.first().map(|u| u.uri.clone()))
                                    .unwrap_or_default();
                                let ft = control.status.file_type.clone();

                                // Drop the lock temporarily so we don't hold the RwLock write-lock while running slow processes like sips/ffmpeg
                                drop(locks);

                                // Perform conversion
                                if !file_path.is_empty() && !url.is_empty() {
                                    manager_clone
                                        .perform_format_conversion(&file_path, &url, ft.as_deref())
                                        .await;
                                }

                                // Re-acquire the write lock to set the status
                                let mut locks = manager_clone.tasks.write().await;
                                if let Some(control) = locks.get_mut(&id_clone) {
                                    control.status.status = "complete".to_string();
                                    let _ =
                                        manager_clone.tx.send(manager_clone.build_notification(
                                            "pin.onDownloadComplete",
                                            &id_clone,
                                        ));
                                }
                            }
                        }
                    }
                    manager_clone.save_session().await;
                }
                Err(e) => {
                    eprintln!("Task '{}' failed: {}", id_clone, e);
                    {
                        let mut locks = manager_clone.tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            control.status.status = "error".to_string();
                        }
                    }
                    let _ = manager_clone
                        .tx
                        .send(manager_clone.build_notification("pin.onDownloadError", &id_clone));
                    manager_clone.save_session().await;
                }
            }
        });
    }

    pub async fn pause_task(&self, id: &str) -> bool {
        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.get_mut(id) {
                control.token.cancel();
                control.status.status = "paused".to_string();
                // Dispatch pause event immediately for UI responsiveness
                let _ = self
                    .tx
                    .send(self.build_notification("pin.onDownloadPause", id));
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
        }
        res
    }

    pub async fn pause_all_tasks(&self) {
        {
            let mut tasks = self.tasks.write().await;
            for (gid, control) in tasks.iter_mut() {
                if control.status.status == "active"
                    || control.status.status == "converting"
                    || control.status.status == "waiting"
                {
                    control.token.cancel();
                    control.status.status = "paused".to_string();
                    let _ = self
                        .tx
                        .send(self.build_notification("pin.onDownloadPause", gid));
                }
            }
        }
        self.save_session().await;
    }

    pub async fn unpause_task(self: &Arc<Self>, id: &str) -> bool {
        let (url, filename, dir, resume_offset, threads, headers) = {
            let tasks = self.tasks.read().await;
            if let Some(control) = tasks.get(id) {
                // If it's already active, don't start it again
                // Unless its token was cancelled (meaning it's in the process of stopping)
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
                let headers: Vec<String> = if let Some(header_str) = control.options.get("header") {
                    header_str
                        .split('\n')
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string())
                        .collect()
                } else {
                    Vec::new()
                };
                (url, filename, dir, resume_offset, threads, headers)
            } else {
                return false;
            }
        };

        self.spawn_task(
            id.to_string(),
            url,
            filename,
            dir,
            threads,
            resume_offset,
            headers,
        )
        .await;
        self.save_session().await;
        true
    }

    pub async fn unpause_all_tasks(self: &Arc<Self>) {
        let gids: Vec<String> = {
            let tasks = self.tasks.read().await;
            tasks
                .iter()
                .filter(|(_, c)| {
                    c.status.status == "paused"
                        || c.status.status == "waiting"
                        || c.status.status == "error"
                        || c.status.status == "removed"
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for gid in gids {
            self.unpause_task(&gid).await;
        }
    }

    pub async fn remove_task(&self, id: &str) -> bool {
        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.remove(id) {
                control.token.cancel();
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
        }
        res
    }

    pub async fn remove_task_and_file(&self, id: &str) -> bool {
        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.remove(id) {
                control.token.cancel();

                // Attempt to move files to trash
                for file in &control.status.files {
                    let path = std::path::Path::new(&file.path);
                    if path.exists() {
                        if let Err(e) = trash::delete(path) {
                            eprintln!(
                                "[ERROR] Failed to move file to trash '{}': {}",
                                file.path, e
                            );
                        } else {
                            println!("[INFO] Moved to trash: {}", file.path);
                        }
                    }
                }
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
        }
        res
    }

    pub async fn force_remove_task(&self, id: &str) -> bool {
        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.remove(id) {
                control.token.cancel();
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
        }
        res
    }

    pub async fn get_task(&self, id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.get(id).map(|c| {
            let mut status = c.status.clone();
            status.download_speed = self.calculate_current_speed(c).to_string();
            status
        })
    }

    pub async fn get_active_tasks(&self) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|c| c.status.status == "active" || c.status.status == "converting")
            .map(|c| {
                let mut status = c.status.clone();
                status.download_speed = self.calculate_current_speed(c).to_string();
                status
            })
            .collect()
    }

    pub async fn get_waiting_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|c| c.status.status == "waiting" || c.status.status == "paused")
            .map(|c| c.status.clone())
            .collect()
    }

    pub async fn get_stopped_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|c| {
                c.status.status == "complete"
                    || c.status.status == "error"
                    || c.status.status == "removed"
            })
            .map(|c| c.status.clone())
            .collect()
    }

    pub async fn get_global_stat(&self) -> GlobalStat {
        let tasks = self.tasks.read().await;
        let mut total_download_speed: u64 = 0;
        let mut active = 0;
        let mut waiting = 0;
        let mut stopped = 0;

        for control in tasks.values() {
            match control.status.status.as_str() {
                "active" | "converting" => {
                    active += 1;
                    total_download_speed += self.calculate_current_speed(control);
                }
                "waiting" | "paused" => waiting += 1,
                "complete" | "error" | "removed" => stopped += 1,
                _ => {}
            }
        }

        let current_mode = {
            let opts = self.global_options.read().await;
            opts.get("speed-mode")
                .cloned()
                .unwrap_or_else(|| "max_bandwidth".to_string())
        };

        if (current_mode == "max_bandwidth" || current_mode == "max")
            && total_download_speed > self.max_seen_speed.load(Ordering::Relaxed)
        {
            self.max_seen_speed
                .store(total_download_speed, Ordering::Relaxed);
        }

        GlobalStat {
            download_speed: total_download_speed.to_string(),
            upload_speed: "0".to_string(),
            num_active: active.to_string(),
            num_waiting: waiting.to_string(),
            num_stopped: stopped.to_string(),
            num_stopped_total: stopped.to_string(),
        }
    }

    pub async fn change_global_option(self: &Arc<Self>, options: HashMap<String, String>) {
        if let Some(mode) = options.get("speed-mode") {
            match mode.as_str() {
                "max_bandwidth" | "max" => {
                    self.current_limit.store(0, Ordering::Relaxed);
                }
                "half_bandwidth" | "half" => {
                    let peak = self.max_seen_speed.load(Ordering::Relaxed);
                    // Use peak / 2, with a fallback floor of 1MB/s if peak is unknown
                    let half_speed = std::cmp::max(peak / 2, 1024 * 1024);
                    self.current_limit.store(half_speed, Ordering::Relaxed);
                }
                "min_bandwidth" | "min" => {
                    // Refined min: Sub-kb/s limit, NO pause
                    self.current_limit.store(768, Ordering::Relaxed); // 768 bytes/s is sub-kb
                }
                _ => {}
            }
        }

        if let Some(limit_str) = options.get("max-overall-download-limit") {
            if let Ok(limit) = limit_str.parse::<u64>() {
                self.current_limit.store(limit, Ordering::Relaxed);
            }
        }

        if let Some(split_str) = options
            .get("split")
            .or_else(|| options.get("default-split"))
        {
            if let Ok(split) = split_str.parse::<u64>() {
                self.default_split.store(split.min(99), Ordering::Relaxed);
            }
        }

        {
            let mut global_opts = self.global_options.write().await;
            for (k, v) in options {
                global_opts.insert(k.clone(), v.clone());
            }
        }
        self.save_session().await;
    }

    pub async fn get_global_option(&self) -> HashMap<String, String> {
        self.global_options.read().await.clone()
    }

    pub async fn change_option(&self, id: &str, options: HashMap<String, String>) -> bool {
        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.get_mut(id) {
                for (k, v) in options {
                    if k == "dir" {
                        control.status.dir = v.clone();
                        if let Some(file) = control.status.files.first_mut() {
                            let file_name = std::path::Path::new(&file.path)
                                .file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "download".to_string());
                            file.path = format!("{}/{}", v, file_name);
                        }
                    }
                    control.options.insert(k, v);
                }
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
        }
        res
    }

    pub async fn get_option(&self, id: &str) -> Option<HashMap<String, String>> {
        let tasks = self.tasks.read().await;
        tasks.get(id).map(|c| c.options.clone())
    }

    pub async fn purge_download_result(&self) {
        let mut tasks = self.tasks.write().await;
        tasks.retain(|_, control| {
            control.status.status != "complete"
                && control.status.status != "error"
                && control.status.status != "removed"
        });
    }

    pub async fn remove_download_result(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get(id) {
            if control.status.status == "complete"
                || control.status.status == "error"
                || control.status.status == "removed"
            {
                tasks.remove(id);
                return true;
            }
        }
        false
    }

    fn calculate_current_speed(&self, control: &TaskControl) -> u64 {
        if control.status.status != "active" && control.status.status != "converting" {
            return 0;
        }

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(control.last_update_time).as_secs_f64();

        let reported_speed = control.status.download_speed.parse::<u64>().unwrap_or(0);

        // If we recently got a chunk (within 1s), use the reported speed
        if elapsed <= 1.0 {
            return reported_speed;
        }

        // Otherwise, recalculate based on wall-clock time since last chunk
        let completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
        let bytes_diff = completed.saturating_sub(control.last_update_bytes);
        (bytes_diff as f64 / elapsed) as u64
    }

    fn build_notification(&self, method: &str, gid: &str) -> String {
        let notification = RPCNotification {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params: vec![NotificationParam {
                gid: gid.to_string(),
            }],
        };
        serde_json::to_string(&notification).unwrap_or_default()
    }

    /// Universal metadata resolver.
    /// 1. Standard Resolution: Fetches HTTP HEAD/GET to read `Content-Length` and `Accept-Ranges`.
    /// 2. Universal Fallback: If a web page is provided, it attempts to scrape OpenGraph tags
    ///    or embedded JSON payloads (e.g., Next.js) to find the actual media URL.
    pub async fn resolve_url(&self, url: String) -> Result<crate::models::ResolveResponse, String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| e.to_string())?;

        // 1. Standard Redirect Resolution (Current Logic)
        let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let final_url = response.url().to_string();

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .to_string();

        let total_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<i64>().ok());

        let is_resumable = response
            .headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .and_then(|h| h.to_str().ok())
            .map(|s| s == "bytes");

        let content_disposition_filename = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                if let Some(idx) = s.find("filename=") {
                    let part = &s[idx + 9..];
                    let name = part
                        .split(';')
                        .next()
                        .unwrap_or(part)
                        .trim()
                        .trim_matches('"');
                    Some(name.to_string())
                } else {
                    None
                }
            });

        // If the result is already a direct video file, return it
        if content_type.contains("video/")
            || final_url.split('?').next().unwrap_or("").ends_with(".mp4")
            || final_url.split('?').next().unwrap_or("").ends_with(".mkv")
        {
            let filename = content_disposition_filename.unwrap_or_else(|| {
                final_url
                    .split('/')
                    .next_back()
                    .unwrap_or("download.bin")
                    .split('?')
                    .next()
                    .unwrap_or("download.bin")
                    .to_string()
            });

            return Ok(crate::models::ResolveResponse {
                url: final_url,
                filename: Some(filename),
                total_size,
                file_type: Some(content_type.to_string()),
                is_resumable,
            });
        }

        // 2. UNIVERSAL FALLBACK: Scrape HTML for metadata
        if content_type.contains("text/html") {
            let html = response.text().await.map_err(|e| e.to_string())?;

            // A. Universal Meta Tags (OpenGraph / Twitter)
            let og_video_re =
                Regex::new(r#"<meta property="(?:og:video|twitter:player)" content="(.*?)"#)
                    .unwrap();
            let og_title_re =
                Regex::new(r#"<meta property="(?:og:title|twitter:title)" content="(.*?)"#)
                    .unwrap();

            if let Some(caps) = og_video_re.captures(&html) {
                let video_url = caps[1].to_string();
                let title = og_title_re
                    .captures(&html)
                    .map(|c| c[1].to_string())
                    .unwrap_or_else(|| "download".to_string());
                return Ok(crate::models::ResolveResponse {
                    url: video_url,
                    filename: Some(format!("{}.mp4", title.replace(" ", "-"))),
                    total_size: None,
                    file_type: Some("video/mp4".to_string()),
                    is_resumable: None,
                });
            }

            // B. Universal JSON Script Extraction (Next.js, Nuxt, etc.)
            let json_re = Regex::new(r#"<script[^>]*type="application/json"[^>]*>(.*?)</script>|<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
            let video_link_re =
                Regex::new(r#"https?://[^\s"\'<>]+?\.(?:mp4|mkv|webm|mov)(?:[^\s"\'<>]*?)"#)
                    .unwrap();

            let mut best_json_link: Option<(String, String)> = None;
            for caps in json_re.captures_iter(&html) {
                let json_content = caps
                    .get(1)
                    .or(caps.get(2))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                if let Ok(data) = serde_json::from_str::<Value>(json_content) {
                    // Heuristic: Search for video links within the JSON structure
                    let json_str = data.to_string();
                    let mut found_links: Vec<String> = video_link_re
                        .find_iter(&json_str)
                        .map(|m| m.as_str().to_string())
                        .collect();
                    if !found_links.is_empty() {
                        // Prioritize links that look like they belong to CDNs or are higher quality
                        found_links.sort_by_key(|a| a.len());
                        if let Some(link) = found_links.last() {
                            best_json_link = Some((link.clone(), "download".to_string()));
                            break;
                        }
                    }
                }
            }
            if let Some((link, _)) = best_json_link {
                // Try to find a title in the HTML as well
                let title = og_title_re
                    .captures(&html)
                    .map(|c| c[1].to_string())
                    .unwrap_or_else(|| "download".to_string());
                let ext = link
                    .split('?')
                    .next()
                    .unwrap_or("")
                    .split('.')
                    .next_back()
                    .unwrap_or("mp4")
                    .to_string();
                return Ok(crate::models::ResolveResponse {
                    url: link,
                    filename: Some(format!("{}.{}", title.replace(" ", "-"), ext)),
                    total_size: None,
                    file_type: Some(format!("video/{}", ext)),
                    is_resumable: None,
                });
            }

            // C. Heuristic Regex Search in Full HTML
            let mut links: Vec<String> = video_link_re
                .find_iter(&html)
                .map(|m| m.as_str().to_string())
                .collect();
            if !links.is_empty() {
                links.sort_by_key(|a| a.len());
                let best_link = links.last().unwrap();
                let name = best_link
                    .split('/')
                    .next_back()
                    .unwrap_or("download.bin")
                    .split('?')
                    .next()
                    .unwrap_or("download.bin");
                return Ok(crate::models::ResolveResponse {
                    url: best_link.to_string(),
                    filename: Some(name.to_string()),
                    total_size: None,
                    file_type: None,
                    is_resumable: None,
                });
            }
        }

        // Final fallback: use the final redirect URL
        let mut filename = content_disposition_filename.unwrap_or_else(|| {
            final_url
                .split('/')
                .next_back()
                .unwrap_or("download.bin")
                .split('?')
                .next()
                .unwrap_or("download.bin")
                .to_string()
        });

        if !filename.contains('.') {
            let ext = match content_type.as_str() {
                "image/jpeg" => "jpg",
                "image/png" => "png",
                "image/gif" => "gif",
                "image/webp" => "webp",
                "application/pdf" => "pdf",
                "application/zip" => "zip",
                "application/x-gzip" => "gz",
                "application/x-tar" => "tar",
                "text/plain" => "txt",
                "text/html" => "html",
                "audio/mpeg" => "mp3",
                "audio/wav" => "wav",
                "audio/ogg" => "ogg",
                "video/mp4" => "mp4",
                "video/x-matroska" => "mkv",
                "video/webm" => "webm",
                _ => "",
            };
            if !ext.is_empty() {
                filename = format!("{}.{}", filename, ext);
            }
        }

        Ok(crate::models::ResolveResponse {
            url: final_url,
            filename: Some(filename),
            total_size,
            file_type: if content_type.is_empty() {
                None
            } else {
                Some(content_type.to_string())
            },
            is_resumable,
        })
    }

    pub async fn perform_format_conversion(
        &self,
        file_path_str: &str,
        url: &str,
        file_type: Option<&str>,
    ) {
        let path = std::path::Path::new(file_path_str);
        if !path.exists() {
            return;
        }

        let target_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if target_ext.is_empty() {
            return;
        }

        // Try to find the source extension from URL
        let mut src_ext = std::path::Path::new(url)
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        // If not found or empty, try from content type
        if src_ext.is_empty() {
            if let Some(ct) = file_type {
                src_ext = match ct {
                    "image/jpeg" | "image/jpg" => "jpg".to_string(),
                    "image/png" => "png".to_string(),
                    "image/gif" => "gif".to_string(),
                    "image/webp" => "webp".to_string(),
                    "image/heic" => "heic".to_string(),
                    "image/heif" => "heif".to_string(),
                    "audio/mpeg" | "audio/mp3" => "mp3".to_string(),
                    "audio/wav" | "audio/x-wav" => "wav".to_string(),
                    "audio/x-m4a" | "audio/m4a" | "audio/x-aac" => "m4a".to_string(),
                    "audio/flac" | "audio/x-flac" => "flac".to_string(),
                    "video/mp4" => "mp4".to_string(),
                    "video/x-matroska" | "video/mkv" => "mkv".to_string(),
                    "video/quicktime" => "mov".to_string(),
                    "video/webm" => "webm".to_string(),
                    _ => "".to_string(),
                };
            }
        }

        if src_ext.is_empty() || src_ext == target_ext {
            // No conversion needed
            return;
        }

        println!(
            "[CONVERTER] Attempting conversion from {} to {}",
            src_ext, target_ext
        );

        let temp_path_str = format!("{}.src.{}", file_path_str, src_ext);
        let temp_path = std::path::Path::new(&temp_path_str);

        // Rename the downloaded file to a temp file containing the raw source bytes
        if let Err(e) = std::fs::rename(path, temp_path) {
            eprintln!(
                "[CONVERTER ERROR] Failed to rename original file to temp path: {}",
                e
            );
            return;
        }

        let mut success = false;
        let images = ["jpg", "jpeg", "png", "webp", "heic", "heif", "pdf"];

        // 2. Image conversion using macOS `sips`
        if !success && images.contains(&src_ext.as_str()) && images.contains(&target_ext.as_str()) {
            let sips_format = match target_ext.as_str() {
                "jpg" | "jpeg" => "jpeg",
                other => other,
            };

            println!(
                "[CONVERTER] Running: sips -s format {} {:?} --out {:?}",
                sips_format, temp_path, path
            );
            match tokio::process::Command::new("sips")
                .arg("-s")
                .arg("format")
                .arg(sips_format)
                .arg(temp_path)
                .arg("--out")
                .arg(path)
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        success = true;
                        println!("[CONVERTER] sips conversion succeeded!");
                    } else {
                        eprintln!(
                            "[CONVERTER ERROR] sips failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("[CONVERTER ERROR] Failed to execute sips: {}", e);
                }
            }
        }

        // 3. Fallback PDF conversion using macOS `cupsfilter`
        if !success && target_ext == "pdf" && images.contains(&src_ext.as_str()) {
            let mime = match src_ext.to_lowercase().as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                _ => "image/png",
            };
            println!(
                "[CONVERTER] Running cupsfilter fallback for PDF conversion: cupsfilter -i {} -m application/pdf {:?}",
                mime, temp_path
            );
            if let Ok(file) = std::fs::File::create(path) {
                match tokio::process::Command::new("cupsfilter")
                    .arg("-i")
                    .arg(mime)
                    .arg("-m")
                    .arg("application/pdf")
                    .arg(temp_path)
                    .stdout(std::process::Stdio::from(file))
                    .output()
                    .await
                {
                    Ok(output) => {
                        if output.status.success() {
                            success = true;
                            println!("[CONVERTER] cupsfilter PDF conversion succeeded!");
                        } else {
                            eprintln!(
                                "[CONVERTER ERROR] cupsfilter failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[CONVERTER ERROR] cupsfilter command failed: {}", e);
                    }
                }
            }
        }

        // 2. Audio/Video conversion using ffmpeg (if available)
        if !success {
            println!(
                "[CONVERTER] Running: ffmpeg -y -i {:?} {:?}",
                temp_path, path
            );
            match tokio::process::Command::new("ffmpeg")
                .arg("-y")
                .arg("-i")
                .arg(temp_path)
                .arg(path)
                .output()
                .await
            {
                Ok(output) => {
                    if output.status.success() {
                        success = true;
                        println!("[CONVERTER] ffmpeg conversion succeeded!");
                    } else {
                        eprintln!(
                            "[CONVERTER ERROR] ffmpeg failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    println!("[CONVERTER] ffmpeg not available: {}", e);
                }
            }
        }

        // 3. Audio fallback using macOS `afconvert`
        let audio_formats = ["mp3", "wav", "m4a", "aac", "flac"];
        if !success
            && audio_formats.contains(&src_ext.as_str())
            && audio_formats.contains(&target_ext.as_str())
        {
            let (af_format, af_data) = match target_ext.as_str() {
                "mp3" => ("mpg3", "wha?"),
                "m4a" | "aac" => ("m4af", "aac "),
                "wav" => ("WAVE", "LEI16"),
                _ => ("", ""),
            };

            if !af_format.is_empty() {
                println!(
                    "[CONVERTER] Running: afconvert -f {} -d {:?} {:?} {:?}",
                    af_format, af_data, temp_path, path
                );
                let mut cmd = tokio::process::Command::new("afconvert");
                cmd.arg("-f").arg(af_format);
                if af_data != "wha?" {
                    cmd.arg("-d").arg(af_data);
                }
                cmd.arg(temp_path).arg(path);

                match cmd.output().await {
                    Ok(output) => {
                        if output.status.success() {
                            success = true;
                            println!("[CONVERTER] afconvert conversion succeeded!");
                        } else {
                            eprintln!(
                                "[CONVERTER ERROR] afconvert failed: {}",
                                String::from_utf8_lossy(&output.stderr)
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[CONVERTER ERROR] Failed to execute afconvert: {}", e);
                    }
                }
            }
        }

        // Clean up or fallback
        if success {
            let _ = std::fs::remove_file(temp_path);
            println!("[CONVERTER] Cleaned up temporary file: {:?}", temp_path);
        } else {
            // Restore original file so no data is lost
            let _ = std::fs::rename(temp_path, path);
            eprintln!("[CONVERTER] Conversion failed or unsupported. Restored original file.");
        }
    }


}
