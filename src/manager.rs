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
use librqbit::{api::TorrentIdOrHash, AddTorrent, AddTorrentOptions, ManagedTorrent, Session};

/// Internal control structure for managing a single download task's state.
/// Holds the cancellation token, specific task options, and live speed calculation tracking data.
struct TaskControl {
    status: TaskStatus,
    token: CancellationToken,
    options: HashMap<String, String>,
    last_update_bytes: u64,
    last_update_time: std::time::Instant,
    expected_hash: Option<String>,
    created_at: u128,
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
    pub torrent_session: tokio::sync::OnceCell<Arc<Session>>,
    pub torrent_handles: RwLock<HashMap<String, Arc<ManagedTorrent>>>,
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
            torrent_session: tokio::sync::OnceCell::new(),
            torrent_handles: RwLock::new(HashMap::new()),
        });
        (manager, rx)
    }

    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    pub async fn get_torrent_session(&self) -> Result<Arc<Session>, String> {
        self.torrent_session
            .get_or_try_init(|| async {
                let home = std::env::var("HOME")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|_| std::env::temp_dir());
                let dir = home.join(".pincer").join("torrents");
                std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

                let opts = librqbit::SessionOptions {
                    listen_port_range: Some(6881..6891),
                    enable_upnp_port_forwarding: false,
                    ..Default::default()
                };
                let session = match Session::new_with_opts(dir.clone(), opts).await {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("[PINCER INFO] Failed to bind standard torrent ports: {:?}. Falling back to ephemeral port.", e);
                        let fallback_opts = librqbit::SessionOptions {
                            listen_port_range: Some(0..1),
                            enable_upnp_port_forwarding: false,
                            ..Default::default()
                        };
                        Session::new_with_opts(dir, fallback_opts)
                            .await
                            .map_err(|err| format!("Failed to create librqbit session with ephemeral port fallback: {:?}", err))?
                    }
                };
                Ok(session)
            })
            .await
            .cloned()
    }

    pub async fn spawn_torrent_task(
        self: &Arc<Self>,
        id: String,
        torrent_source: AddTorrent<'static>,
        dir: String,
        options: HashMap<String, String>,
    ) -> Result<String, String> {
        let session = self.get_torrent_session().await?;
        println!(
            "[PINCER OUT] spawn_torrent_task called for id: {}, dir: {}",
            id, dir
        );

        let (initial_name, initial_info_hash) = match &torrent_source {
            AddTorrent::Url(url) => {
                let name = if url.starts_with("magnet:?") {
                    if let Some(pos) = url.find("dn=") {
                        let rest = &url[pos + 3..];
                        let end = rest.find('&').unwrap_or(rest.len());
                        percent_encoding::percent_decode_str(&rest[..end])
                            .decode_utf8()
                            .map(|s| s.into_owned())
                            .unwrap_or_else(|_| "Magnet Link".to_string())
                    } else {
                        "Magnet Link".to_string()
                    }
                } else {
                    url.split('/')
                        .next_back()
                        .and_then(|s| s.split('?').next())
                        .unwrap_or("Torrent Link")
                        .to_string()
                };

                let info_hash = if let Some(pos) = url.find("urn:btih:") {
                    let start = pos + 9;
                    let rest = &url[start..];
                    let end = rest.find('&').unwrap_or(rest.len());
                    Some(rest[..end].to_lowercase())
                } else {
                    None
                };

                (name, info_hash)
            }
            AddTorrent::TorrentFileBytes(bytes) => {
                if let Ok(t) = librqbit::torrent_from_bytes::<&[u8]>(bytes.as_ref()) {
                    let name = t
                        .info
                        .name
                        .as_ref()
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_else(|| "Torrent File".to_string());
                    let info_hash = Some(t.info_hash.as_string());
                    (name, info_hash)
                } else {
                    ("Torrent File".to_string(), None)
                }
            }
        };
        let source_url = match &torrent_source {
            AddTorrent::Url(u) => Some(u.to_string()),
            _ => None,
        };

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
            bittorrent: Some(crate::models::TorrentInfo {
                announce_list: vec![],
                comment: None,
                creation_date: None,
                mode: "single".to_string(),
                info: crate::models::TorrentInfoInner {
                    name: initial_name.clone(),
                },
            }),
            info_hash: initial_info_hash,
            num_seeders: Some(0),
            url: source_url,
        };

        let token = CancellationToken::new();

        {
            let mut tasks = self.tasks.write().await;
            let mut opts = options.clone();
            opts.insert("dir".to_string(), dir.clone());
            opts.insert("out".to_string(), initial_name.clone());
            tasks.insert(
                id.clone(),
                TaskControl {
                    status: initial_status,
                    token: token.clone(),
                    options: opts,
                    last_update_bytes: 0,
                    last_update_time: std::time::Instant::now(),
                    expected_hash: None,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_millis(0))
                        .as_millis(),
                },
            );
        }

        let _ = self
            .tx
            .send(self.build_notification("pin.onDownloadStart", &id));

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

        let manager_clone = self.clone();
        let id_clone = id.clone();
        let session_clone = session.clone();
        let dir_clone = dir.clone();

        let torrent_source_clone = match &torrent_source {
            librqbit::AddTorrent::Url(cow) => librqbit::AddTorrent::Url(cow.clone()),
            librqbit::AddTorrent::TorrentFileBytes(bytes) => {
                librqbit::AddTorrent::TorrentFileBytes(bytes.clone())
            }
        };

        tokio::spawn(async move {
            let mut resolved_name = initial_name.clone();
            let mut is_multi = false;

            match &torrent_source_clone {
                librqbit::AddTorrent::TorrentFileBytes(bytes) => {
                    if let Ok(t) = librqbit::torrent_from_bytes::<&[u8]>(bytes.as_ref()) {
                        is_multi = t.info.files.is_some();
                        if let Some(name_buf) = &t.info.name {
                            resolved_name = String::from_utf8_lossy(name_buf).into_owned();
                        }
                    }
                }
                librqbit::AddTorrent::Url(url) => {
                    let list_opts = librqbit::AddTorrentOptions {
                        list_only: true,
                        ..Default::default()
                    };
                    let source_for_add = librqbit::AddTorrent::Url(url.clone());
                    if let Ok(librqbit::AddTorrentResponse::ListOnly(res)) = session_clone
                        .add_torrent(source_for_add, Some(list_opts))
                        .await
                    {
                        is_multi = res.info.files.is_some();
                        if let Some(name_buf) = &res.info.name {
                            resolved_name = String::from_utf8_lossy(name_buf).into_owned();
                        }
                    }
                }
            }

            let folder_name = if !is_multi {
                let path = std::path::Path::new(&resolved_name);
                path.file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| resolved_name.clone())
            } else {
                resolved_name.clone()
            };

            let folder_name = folder_name.replace(' ', "-").replace('.', "_");

            let final_path = std::path::PathBuf::from(&dir_clone).join(&folder_name);
            let _ = std::fs::create_dir_all(&final_path);
            let final_dir = final_path.to_string_lossy().to_string();

            {
                let mut tasks = manager_clone.tasks.write().await;
                if let Some(control) = tasks.get_mut(&id_clone) {
                    control.status.dir = final_dir.clone();
                    control.options.insert("dir".to_string(), final_dir.clone());
                    control
                        .options
                        .insert("out".to_string(), resolved_name.clone());
                }
            }

            let opts = AddTorrentOptions {
                overwrite: true,
                output_folder: Some(final_dir),
                only_files,
                ..Default::default()
            };

            println!(
                "[PINCER OUT] Calling add_torrent in background for id={}",
                id_clone
            );
            match session_clone.add_torrent(torrent_source, Some(opts)).await {
                Ok(add_res) => {
                    println!("[PINCER OUT] add_torrent succeeded for id={}", id_clone);
                    if let Some(handle) = add_res.into_handle() {
                        {
                            let mut handles = manager_clone.torrent_handles.write().await;
                            handles.insert(id_clone.clone(), handle.clone());
                        }

                        let is_paused = {
                            let tasks = manager_clone.tasks.read().await;
                            tasks
                                .get(&id_clone)
                                .map(|c| c.status.status.as_str() == "paused")
                                .unwrap_or(false)
                        };

                        if is_paused {
                            println!("[PINCER OUT] Task was paused during initialization, pausing handle for id={}", id_clone);
                            let _ = session_clone.pause(&handle).await;
                        } else {
                            let current_token = {
                                let tasks = manager_clone.tasks.read().await;
                                tasks.get(&id_clone).map(|c| c.token.clone())
                            };
                            if let Some(tok) = current_token {
                                manager_clone
                                    .run_torrent_stats_loop(id_clone, handle, tok)
                                    .await;
                            }
                        }
                    } else {
                        eprintln!("[PINCER ERR] add_torrent succeeded but into_handle returned None for id={}", id_clone);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[PINCER ERR] add_torrent failed for id={}: {:?}",
                        id_clone, e
                    );
                    let mut tasks = manager_clone.tasks.write().await;
                    if let Some(control) = tasks.get_mut(&id_clone) {
                        control.status.status = "error".to_string();
                    }
                }
            }
        });

        self.save_session().await;
        Ok(id)
    }

    pub async fn run_torrent_stats_loop(
        self: Arc<Self>,
        id: String,
        handle: Arc<ManagedTorrent>,
        token: CancellationToken,
    ) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    break;
                }
                _ = interval.tick() => {
                    let stats = handle.stats();
                    let total_bytes = stats.total_bytes;
                    let progress_bytes = stats.progress_bytes;
                    let has_metadata = handle.metadata.load().is_some();
                    let finished = stats.finished && has_metadata;

                    let mut download_speed = 0;
                    let mut upload_speed = 0;
                    let mut peer_count = 0;
                    if let Some(live) = &stats.live {
                        download_speed = (live.download_speed.mbps * 1024.0 * 1024.0) as u64;
                        upload_speed = (live.upload_speed.mbps * 1024.0 * 1024.0) as u64;
                        peer_count = live.snapshot.peer_stats.live;
                    }

                    let dir = {
                        let tasks = self.tasks.read().await;
                        tasks.get(&id).map(|c| c.status.dir.clone()).unwrap_or_default()
                    };

                    let files = {
                        let mut fls = Vec::new();
                        if let Some(meta) = &*handle.metadata.load() {
                            let parent_dir = std::path::PathBuf::from(&dir);
                            if let Some(files) = &meta.info.files {
                                for f in files {
                                    let mut file_path = parent_dir.clone();
                                    for component in &f.path {
                                        file_path.push(String::from_utf8_lossy(component.as_ref()).as_ref());
                                    }
                                    fls.push(crate::models::FileData {
                                        path: file_path.to_string_lossy().to_string(),
                                        uris: vec![],
                                    });
                                }
                            } else {
                                let mut file_path = parent_dir.clone();
                                if let Some(name_buf) = &meta.info.name {
                                    file_path.push(String::from_utf8_lossy(name_buf.as_ref()).as_ref());
                                }
                                fls.push(crate::models::FileData {
                                    path: file_path.to_string_lossy().to_string(),
                                    uris: vec![],
                                });
                            }
                        }
                        fls
                    };

                    let bittorrent = if let Some(meta) = &*handle.metadata.load() {
                        let mode = if meta.info.files.is_some() { "multi" } else { "single" };
                        let name = meta.info.name.as_ref()
                            .map(|b| String::from_utf8_lossy(b.as_ref()).to_string())
                            .unwrap_or_default();
                        let mut announce_list = Vec::new();
                        for tracker in &handle.shared.trackers {
                            announce_list.push(vec![tracker.to_string()]);
                        }
                        Some(crate::models::TorrentInfo {
                            announce_list,
                            comment: None,
                            creation_date: None,
                            mode: mode.to_string(),
                            info: crate::models::TorrentInfoInner { name },
                        })
                    } else {
                        None
                    };

                    let mut task_finished = false;
                    let mut keep_seeding = false;
                    {
                        let mut tasks = self.tasks.write().await;
                        if let Some(control) = tasks.get_mut(&id) {
                            control.status.total_length = total_bytes.to_string();
                            control.status.completed_length = progress_bytes.to_string();
                            control.status.download_speed = download_speed.to_string();
                            control.status.upload_speed = Some(upload_speed.to_string());
                            control.status.num_seeders = Some(peer_count as u32);
                            control.status.bittorrent = bittorrent;
                            if !files.is_empty() {
                                control.status.files = files;
                            }
                            if finished {
                                control.status.status = "complete".to_string();
                                task_finished = true;
                                keep_seeding = control.options.get("keep-seeding").map(|s| s == "true").unwrap_or(false);
                            }
                        }
                    }

                    if task_finished && !keep_seeding {
                        if let Ok(sess) = self.get_torrent_session().await {
                            let _ = sess.pause(&handle).await;
                        }
                        let mut tasks = self.tasks.write().await;
                        if let Some(control) = tasks.get_mut(&id) {
                            control.status.status = "paused".to_string();
                        }
                    }

                    self.save_session().await;
                    let _ = self.tx.send(self.build_notification("pin.onDownloadProgress", &id));

                    if task_finished {
                        let mut unselected_files = Vec::new();
                        {
                            let tasks = self.tasks.read().await;
                            if let Some(control) = tasks.get(&id) {
                                if let Some(files_str) = control.options.get("select-files") {
                                    let selected_indices: std::collections::HashSet<usize> = files_str
                                        .split(',')
                                        .filter_map(|s| s.parse::<usize>().ok())
                                        .collect();
                                    
                                    if let Some(meta) = &*handle.metadata.load() {
                                        let parent_dir = std::path::PathBuf::from(&dir);
                                        if let Some(files) = &meta.info.files {
                                            for (idx, f) in files.iter().enumerate() {
                                                if !selected_indices.contains(&idx) {
                                                    let mut file_path = parent_dir.clone();
                                                    for component in &f.path {
                                                        file_path.push(String::from_utf8_lossy(component.as_ref()).as_ref());
                                                    }
                                                    unselected_files.push(file_path);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        for path in unselected_files {
                            if path.is_file() {
                                if let Err(e) = std::fs::remove_file(&path) {
                                    eprintln!("[PINCER ERR] Failed to delete unselected torrent file {:?}: {:?}", path, e);
                                } else {
                                    println!("[PINCER INFO] Deleted unselected torrent file {:?}", path);
                                    let mut parent = path.parent();
                                    let torrent_dir = std::path::Path::new(&dir);
                                    while let Some(p) = parent {
                                        if p == torrent_dir {
                                            break;
                                        }
                                        if p.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false) {
                                            if let Err(e) = std::fs::remove_dir(p) {
                                                eprintln!("[PINCER ERR] Failed to delete empty directory {:?}: {:?}", p, e);
                                                break;
                                            }
                                            parent = p.parent();
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let _ = self.tx.send(self.build_notification("pin.onDownloadComplete", &id));
                        break;
                    }
                }
            }
        }
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

            let mut opt_url = None;
            let mut opt_total = None;
            let mut opt_completed = None;

            let is_torrent = status.file_type.as_deref() == Some("torrent");
            if saved_status == "complete" || saved_status == "error" || is_torrent {
                opt_url = Some(first_uri);
                opt_total = Some(status.total_length.parse::<u64>().unwrap_or(0));
                opt_completed = Some(status.completed_length.parse::<u64>().unwrap_or(0));
            } else {
                // For active/paused tasks, save state into the .download bundle
                let threads = control
                    .options
                    .get("split")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(4);

                let bundle_state = crate::models::BundleState {
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
                if !std::path::Path::new(&bundle_dir).exists() {
                    let _ = std::fs::create_dir_all(&bundle_dir);
                }

                let state_path = format!("{}/state.json", bundle_dir);
                if let Ok(json_str) = serde_json::to_string_pretty(&bundle_state) {
                    if let Err(e) = std::fs::write(&state_path, json_str) {
                        eprintln!("Failed to write state.json: {}", e);
                    }
                }
            }

            session_tasks.push(crate::models::SessionTask {
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

                    let mut task_status = task.status.clone();
                    let mut bundle_state_opt = None;

                    let is_torrent = task.file_type.as_deref() == Some("torrent");
                    if task_status != "complete" && task_status != "error" && !is_torrent {
                        let state_path =
                            format!("{}/{}.download/state.json", task.save_path, task.filename);
                        if let Ok(json_str) = std::fs::read_to_string(&state_path) {
                            if let Ok(b) =
                                serde_json::from_str::<crate::models::BundleState>(&json_str)
                            {
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
                                expected_hash: None,
                                created_at: task.created_at,
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
        let sanitized = crate::models::sanitize_filename(filename);
        let tasks = self.tasks.read().await;
        let mut unique_name = sanitized.clone();
        let mut counter = 1;

        let path = std::path::Path::new(&sanitized);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&sanitized);
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
                expected_hash: None,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or(std::time::Duration::from_millis(0))
                    .as_millis(),
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
        urls: Vec<String>,
        mut filename: String,
        dir: String,
        threads: usize,
        resume_offset: u64,
        headers: Vec<String>,
        expected_hash: Option<String>,
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
        let mut initial_status = TaskStatus {
            gid: id.clone(),
            status: "waiting".to_string(),
            total_length: "0".to_string(), // Will be updated by task.start
            completed_length,
            download_speed: "0".to_string(),
            upload_speed: None,
            worker_progress: vec![0; threads],
            file_type: None,
            is_resumable: None,
            files: vec![FileData {
                path: format!("{}/{}", dir, filename),
                uris: urls.iter().map(|u| FileUri { uri: u.clone() }).collect(),
            }],
            dir: dir.clone(),
            bittorrent: None,
            info_hash: None,
            num_seeders: None,
            url: urls.first().cloned(),
        };

        {
            let tasks = self.tasks.read().await;
            if let Some(existing) = tasks.get(&id) {
                initial_status.total_length = existing.status.total_length.clone();
                initial_status.file_type = existing.status.file_type.clone();
                initial_status.is_resumable = existing.status.is_resumable;

                let mut wp = existing.status.worker_progress.clone();
                if wp.len() == threads {
                    initial_status.worker_progress = wp.clone();
                } else {
                    wp.resize(threads, 0);
                    initial_status.worker_progress = wp.clone();
                }
            }
        }

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
                    expected_hash,
                    created_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or(std::time::Duration::from_millis(0))
                        .as_millis(),
                },
            );
        }

        self.save_session().await;
        self.schedule_tasks().await;
    }

    pub fn schedule_tasks<'a>(
        self: &'a Arc<Self>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            let max_concurrent = {
                let opts = self.global_options.read().await;
                opts.get("max-concurrent-downloads")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(5)
            };

            let mut active_count = 0;
            let mut waiting_tasks = Vec::new();

            {
                let tasks = self.tasks.read().await;
                for (gid, control) in tasks.iter() {
                    if control.status.status == "active" || control.status.status == "converting" {
                        active_count += 1;
                    } else if control.status.status == "waiting" {
                        waiting_tasks.push((gid.clone(), control.created_at));
                    }
                }
            }

            waiting_tasks.sort_by_key(|(_, created_at)| *created_at);

            for (gid, _) in waiting_tasks {
                if max_concurrent > 0 && active_count >= max_concurrent {
                    break;
                }
                self.execute_task(gid).await;
                active_count += 1;
            }
        })
    }

    pub async fn execute_task(self: &Arc<Self>, id: String) {
        let (urls, filename, dir, threads, existing_worker_progress, headers, expected_hash, token) = {
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
                let existing_worker_progress = control.status.worker_progress.clone();
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
                    existing_worker_progress,
                    headers,
                    expected_hash,
                    token,
                )
            } else {
                return;
            }
        };

        let _ = self
            .tx
            .send(self.build_notification("pin.onDownloadStart", &id));

        // 2. Spawn the background rust task
        let manager_clone = self.clone();
        let id_clone = id.clone();
        let global_opts = self.global_options.read().await.clone();
        tokio::spawn(async move {
            let task = crate::task::DownloadTask {
                urls,
                filename: filename.clone(),
                save_path: dir.clone(),
                threads,
                worker_progress: existing_worker_progress,
                headers,
                global_limit: manager_clone.current_limit.clone(),
                active_threads: manager_clone.active_threads.clone(),
                global_options: global_opts,
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
                        let mut locks = manager_clone.tasks.write().await;
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
                                let completed =
                                    control.status.completed_length.parse::<u64>().unwrap_or(0);
                                let total = control.status.total_length.parse::<u64>().unwrap_or(0);

                                if total > 0 && completed < total {
                                    control.status.status = "error".to_string();
                                    let _ = manager_clone.tx.send(
                                        manager_clone
                                            .build_notification("pin.onDownloadError", &id_clone),
                                    );
                                } else {
                                    // Extract file path (still the .pincer file at this point), URL and file type for conversion
                                    let file_path = format!("{}/{}", dir, part_filename);
                                    let url = control
                                        .status
                                        .files
                                        .first()
                                        .and_then(|f| f.uris.first().map(|u| u.uri.clone()))
                                        .unwrap_or_default();
                                    let ft = control.status.file_type.clone();
                                    let final_path = format!("{}/{}", dir, filename);

                                    // Drop the lock temporarily so we don't hold the RwLock write-lock while running slow processes like sips/ffmpeg
                                    drop(locks);

                                    let mut hash_valid = true;
                                    if let Some(expected) = &expected_hash {
                                        let file_path_clone = file_path.clone();
                                        let expected_clone = expected.clone();
                                        match tokio::task::spawn_blocking(move || {
                                            use sha2::{Digest, Sha256};
                                            let mut file = std::fs::File::open(&file_path_clone)?;
                                            let mut hasher = Sha256::new();
                                            std::io::copy(&mut file, &mut hasher)?;
                                            let hash = hasher.finalize();
                                            Ok::<String, std::io::Error>(hex::encode(hash))
                                        })
                                        .await
                                        {
                                            Ok(Ok(actual_hash)) => {
                                                if actual_hash.to_lowercase()
                                                    != expected_clone.to_lowercase()
                                                {
                                                    eprintln!(
                                                        "Hash mismatch! Expected: {}, Actual: {}",
                                                        expected_clone, actual_hash
                                                    );
                                                    hash_valid = false;
                                                }
                                            }
                                            _ => {
                                                eprintln!("Failed to calculate file hash.");
                                                hash_valid = false;
                                            }
                                        }
                                    }

                                    if !hash_valid {
                                        // Cleanup the .pincer file on hash failure
                                        let _ = std::fs::remove_file(&file_path);
                                        let mut locks = manager_clone.tasks.write().await;
                                        if let Some(control) = locks.get_mut(&id_clone) {
                                            control.status.status = "error".to_string();
                                            let _ = manager_clone.tx.send(
                                                manager_clone.build_notification(
                                                    "pin.onDownloadError",
                                                    &id_clone,
                                                ),
                                            );
                                        }
                                        return;
                                    }

                                    // If a .part file was used inside a bundle, rename it back to final before conversion
                                    if std::path::Path::new(&file_path).exists()
                                        && file_path != final_path
                                    {
                                        if let Err(e) = std::fs::rename(&file_path, &final_path) {
                                            eprintln!(
                                                "Failed to rename pincer file to final file: {}",
                                                e
                                            );
                                        }
                                    }

                                    // Clean up the residual .download bundle directory
                                    let bundle_path = format!("{}/{}.download", dir, filename);
                                    if std::path::Path::new(&bundle_path).exists() {
                                        if let Err(e) = std::fs::remove_dir_all(&bundle_path) {
                                            eprintln!(
                                                "Warning: Failed to remove .download bundle: {}",
                                                e
                                            );
                                        }
                                    }

                                    // Perform conversion on the final path
                                    if !final_path.is_empty() && !url.is_empty() {
                                        if let Err(e) = manager_clone
                                            .perform_format_conversion(
                                                &final_path,
                                                &url,
                                                ft.as_deref(),
                                            )
                                            .await
                                        {
                                            eprintln!("Conversion error for {}: {}", final_path, e);
                                            let mut locks = manager_clone.tasks.write().await;
                                            if let Some(control) = locks.get_mut(&id_clone) {
                                                control.status.status = "error".to_string();
                                                // Optional: store error message somewhere if supported
                                                let _ = manager_clone.tx.send(
                                                    manager_clone.build_notification(
                                                        "pin.onDownloadError",
                                                        &id_clone,
                                                    ),
                                                );
                                            }
                                            return;
                                        }
                                    }

                                    // Remove quarantine xattr now that the download is fully complete
                                    #[cfg(target_os = "macos")]
                                    if let Err(e) =
                                        xattr::remove(&final_path, "com.apple.quarantine")
                                    {
                                        eprintln!(
                                            "Warning: Failed to remove quarantine xattr: {}",
                                            e
                                        );
                                    }

                                    // Re-acquire the write lock to set the status
                                    let mut locks = manager_clone.tasks.write().await;
                                    if let Some(control) = locks.get_mut(&id_clone) {
                                        control.status.status = "complete".to_string();
                                        let _ = manager_clone.tx.send(
                                            manager_clone.build_notification(
                                                "pin.onDownloadComplete",
                                                &id_clone,
                                            ),
                                        );
                                    }
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
                            // Clean up the .download bundle on unrecoverable error
                            let bundle_path = format!("{}/{}.download", dir, filename);
                            let _ = std::fs::remove_dir_all(&bundle_path);
                            let final_path = format!("{}/{}", dir, filename);
                            let _ = std::fs::remove_file(&final_path);
                        }
                    }
                    let _ = manager_clone
                        .tx
                        .send(manager_clone.build_notification("pin.onDownloadError", &id_clone));
                    manager_clone.save_session().await;
                }
            }

            // Check queue to see if more tasks can be spawned
            manager_clone.schedule_tasks().await;
        });
    }

    pub async fn pause_task(self: &Arc<Self>, id: &str) -> bool {
        let (is_torrent, is_completed_torrent) = {
            let tasks = self.tasks.read().await;
            if let Some(c) = tasks.get(id) {
                let is_tor = c.status.file_type.as_deref() == Some("torrent");
                let completed = c.status.completed_length.parse::<u64>().unwrap_or(0);
                let total = c.status.total_length.parse::<u64>().unwrap_or(0);
                let is_comp = total > 0 && completed >= total;
                (is_tor, is_comp)
            } else {
                (false, false)
            }
        };

        if is_torrent {
            if is_completed_torrent {
                return false;
            }
            let handle = {
                let handles = self.torrent_handles.read().await;
                handles.get(id).cloned()
            };
            if let Some(handle) = handle {
                if let Ok(session) = self.get_torrent_session().await {
                    let _ = session.pause(&handle).await;
                }
            }
        }

        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.get_mut(id) {
                control.token.cancel();
                control.status.status = "paused".to_string();
                control.status.download_speed = "0".to_string();
                control.status.upload_speed = Some("0".to_string());
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
            self.schedule_tasks().await;
        }
        res
    }

    pub async fn pause_all_tasks(self: &Arc<Self>) {
        {
            let mut tasks = self.tasks.write().await;
            for (gid, control) in tasks.iter_mut() {
                let is_completed_torrent = control.status.file_type.as_deref() == Some("torrent") && {
                    let completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
                    let total = control.status.total_length.parse::<u64>().unwrap_or(0);
                    total > 0 && completed >= total
                };

                if is_completed_torrent {
                    continue;
                }

                if control.status.status == "active"
                    || control.status.status == "converting"
                    || control.status.status == "waiting"
                {
                    control.token.cancel();
                    control.status.status = "paused".to_string();
                    control.status.download_speed = "0".to_string();
                    control.status.upload_speed = Some("0".to_string());
                    let _ = self
                        .tx
                        .send(self.build_notification("pin.onDownloadPause", gid));
                }
            }
        }
        self.save_session().await;
        self.schedule_tasks().await;
    }

    pub async fn unpause_task(self: &Arc<Self>, id: &str) -> bool {
        let (is_torrent, is_completed_torrent) = {
            let tasks = self.tasks.read().await;
            if let Some(c) = tasks.get(id) {
                let is_tor = c.status.file_type.as_deref() == Some("torrent");
                let completed = c.status.completed_length.parse::<u64>().unwrap_or(0);
                let total = c.status.total_length.parse::<u64>().unwrap_or(0);
                let is_comp = total > 0 && completed >= total;
                (is_tor, is_comp)
            } else {
                (false, false)
            }
        };

        if is_torrent {
            if is_completed_torrent {
                return false;
            }
            let handle = {
                let handles = self.torrent_handles.read().await;
                handles.get(id).cloned()
            };
            if let Some(handle) = handle {
                if let Ok(session) = self.get_torrent_session().await {
                    match session.unpause(&handle).await {
                        Ok(_) => {
                            let token = CancellationToken::new();
                            {
                                let mut tasks = self.tasks.write().await;
                                if let Some(control) = tasks.get_mut(id) {
                                    control.status.status = "active".to_string();
                                    control.token = token.clone();
                                }
                            }

                            let manager_clone = self.clone();
                            let id_clone = id.to_string();
                            let handle_clone = handle.clone();
                            let token_clone = token.clone();
                            tokio::spawn(async move {
                                manager_clone
                                    .run_torrent_stats_loop(id_clone, handle_clone, token_clone)
                                    .await;
                            });

                            let _ = self
                                .tx
                                .send(self.build_notification("pin.onDownloadStart", id));
                            self.save_session().await;
                            return true;
                        }
                        Err(e) => {
                            eprintln!("[PINCER ERR] session.unpause failed for id={}: {:?}", id, e);
                        }
                    }
                }
            } else {
                let token = CancellationToken::new();
                let mut updated = false;
                {
                    let mut tasks = self.tasks.write().await;
                    if let Some(control) = tasks.get_mut(id) {
                        control.status.status = "active".to_string();
                        control.token = token;
                        control
                            .options
                            .insert("keep-seeding".to_string(), "true".to_string());
                        updated = true;
                    }
                }
                if updated {
                    self.save_session().await;
                    return true;
                }
            }
            return false;
        }

        let (url, filename, dir, mut resume_offset, threads, headers, is_resumable) = {
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
            // Delete the dummy target file, staging .download bundle directory, and hidden pincer file so the worker starts fresh
            let path_str = format!("{}/{}", dir, filename);
            let bundle_path_str = format!("{}/{}.download", dir, filename);
            let part_path_str = format!("{}/.{}.pincer", dir, filename);
            let path = std::path::Path::new(&path_str);
            let bundle_path = std::path::Path::new(&bundle_path_str);
            let part_path = std::path::Path::new(&part_path_str);
            if path.exists() {
                if let Err(e) = std::fs::remove_file(path) {
                    eprintln!("[ERROR] Failed to permanently remove non-resumable dummy file before restart: {}", e);
                } else {
                    println!(
                        "[INFO] Permanently removed non-resumable dummy file for fast restart: {}",
                        path.display()
                    );
                }
            }
            if bundle_path.exists() {
                if let Err(e) = std::fs::remove_dir_all(bundle_path) {
                    eprintln!("[ERROR] Failed to permanently remove non-resumable staging bundle directory before restart: {}", e);
                } else {
                    println!(
                        "[INFO] Permanently removed non-resumable staging bundle directory for fast restart: {}",
                        bundle_path.display()
                    );
                }
            }
            if part_path.exists() {
                let _ = std::fs::remove_file(part_path);
            }
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

    pub async fn unpause_all_tasks(self: &Arc<Self>) {
        let gids: Vec<String> = {
            let tasks = self.tasks.read().await;
            tasks
                .iter()
                .filter(|(_, c)| {
                    let is_completed_torrent = c.status.file_type.as_deref() == Some("torrent") && {
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

    pub async fn change_uri(
        &self,
        id: &str,
        file_index: usize,
        del_uris: Vec<String>,
        add_uris: Vec<String>,
    ) -> Result<(usize, usize), String> {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
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

    pub async fn remove_task(self: &Arc<Self>, id: &str) -> bool {
        let is_torrent = {
            let tasks = self.tasks.read().await;
            tasks.get(id).and_then(|c| c.status.file_type.clone()) == Some("torrent".to_string())
        };

        if is_torrent {
            let handle = {
                let mut handles = self.torrent_handles.write().await;
                handles.remove(id)
            };
            if let Some(handle) = handle {
                if let Ok(session) = self.get_torrent_session().await {
                    let _ = session
                        .delete(TorrentIdOrHash::Id(handle.id()), false)
                        .await;
                }
            }
        }

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
            self.schedule_tasks().await;
        }
        res
    }

    pub async fn remove_task_and_file(self: &Arc<Self>, id: &str) -> bool {
        let is_torrent = {
            let tasks = self.tasks.read().await;
            tasks.get(id).and_then(|c| c.status.file_type.clone()) == Some("torrent".to_string())
        };

        if is_torrent {
            let handle = {
                let mut handles = self.torrent_handles.write().await;
                handles.remove(id)
            };
            if let Some(handle) = handle {
                if let Ok(session) = self.get_torrent_session().await {
                    let _ = session
                        .delete(TorrentIdOrHash::Id(handle.id()), false)
                        .await;
                }
            }
        }

        let res = {
            let mut tasks = self.tasks.write().await;
            if let Some(control) = tasks.remove(id) {
                control.token.cancel();

                // Move files to trash in a background thread to prevent blocking the RPC loop
                let is_torrent_task = control.status.file_type.as_deref() == Some("torrent");
                let status = control.status.status.clone();
                let is_resumable = control.status.is_resumable;
                let files = control.status.files.clone();
                let dir = control.status.dir.clone();

                let torrent_folder_or_file = if is_torrent_task {
                    Some((std::path::PathBuf::from(&dir), true))
                } else {
                    None
                };

                tokio::task::spawn_blocking(move || {
                    if is_torrent_task {
                        let mut resolved_torrent_delete = false;
                        if let Some((path, is_dir)) = torrent_folder_or_file.as_ref() {
                            let mut is_safe = true;
                            if let Ok(home) = std::env::var("HOME").map(std::path::PathBuf::from) {
                                if path == &home || path == &home.join("Downloads") {
                                    is_safe = false;
                                }
                            }
                            if is_safe && *is_dir && path.exists() {
                                resolved_torrent_delete = true;
                                let trash_res = std::panic::catch_unwind(|| trash::delete(path));
                                match trash_res {
                                    Ok(Ok(())) => {
                                        println!(
                                            "[INFO] Moved torrent path to trash: {}",
                                            path.display()
                                        );
                                    }
                                    _ => {
                                        eprintln!("[WARNING] Failed to move torrent to trash. Deleting permanently: {}", path.display());
                                        let _ = std::fs::remove_dir_all(path);
                                    }
                                }
                            }
                        }

                        if !resolved_torrent_delete {
                            if !files.is_empty() {
                                for file in &files {
                                    let path = std::path::Path::new(&file.path);
                                    if path.exists() {
                                        let trash_res =
                                            std::panic::catch_unwind(|| trash::delete(path));
                                        match trash_res {
                                            Ok(Ok(())) => {
                                                println!(
                                                    "[INFO] Moved torrent file to trash: {}",
                                                    path.display()
                                                );
                                            }
                                            _ => {
                                                eprintln!("[WARNING] Failed to move torrent file to trash. Deleting permanently: {}", path.display());
                                                let _ = std::fs::remove_file(path);
                                            }
                                        }
                                    }
                                }
                            } else if let Some((path, _)) = torrent_folder_or_file.as_ref() {
                                if path.exists() {
                                    let trash_res =
                                        std::panic::catch_unwind(|| trash::delete(path));
                                    match trash_res {
                                        Ok(Ok(())) => {
                                            println!(
                                                "[INFO] Moved torrent file to trash (fallback): {}",
                                                path.display()
                                            );
                                        }
                                        _ => {
                                            eprintln!("[WARNING] Failed to move torrent file to trash (fallback). Deleting permanently: {}", path.display());
                                            let _ = std::fs::remove_file(path);
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        for file in &files {
                            let path = std::path::Path::new(&file.path);
                            let filename = path.file_name().unwrap_or_default().to_string_lossy();
                            let file_dir = path
                                .parent()
                                .unwrap_or_else(|| std::path::Path::new(""))
                                .to_string_lossy();
                            let bundle_path_str = format!("{}/{}.download", file_dir, filename);
                            let bundle_path = std::path::Path::new(&bundle_path_str);
                            let part_path_str = format!("{}/.{}.pincer", file_dir, filename);
                            let part_path = std::path::Path::new(&part_path_str);

                            let paths_to_remove = vec![
                                path.to_path_buf(),
                                bundle_path.to_path_buf(),
                                part_path.to_path_buf(),
                            ];
                            for p in paths_to_remove {
                                if p.exists() {
                                    let is_dir = p.is_dir();
                                    if is_resumable == Some(false) && status != "complete" {
                                        if is_dir {
                                            let _ = std::fs::remove_dir_all(&p);
                                        } else {
                                            let _ = std::fs::remove_file(&p);
                                        }
                                    } else {
                                        let trash_res =
                                            std::panic::catch_unwind(|| trash::delete(&p));
                                        match trash_res {
                                            Ok(Ok(())) => {
                                                println!(
                                                    "[INFO] Moved file/directory to trash: {}",
                                                    p.display()
                                                );
                                            }
                                            _ => {
                                                eprintln!("[WARNING] Failed to move file/directory to trash. Deleting permanently: {}", p.display());
                                                if is_dir {
                                                    let _ = std::fs::remove_dir_all(&p);
                                                } else {
                                                    let _ = std::fs::remove_file(&p);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
                true
            } else {
                false
            }
        };
        if res {
            self.save_session().await;
            self.schedule_tasks().await;
        }
        res
    }

    pub async fn force_remove_task(self: &Arc<Self>, id: &str) -> bool {
        let is_torrent = {
            let tasks = self.tasks.read().await;
            tasks.get(id).and_then(|c| c.status.file_type.clone()) == Some("torrent".to_string())
        };

        if is_torrent {
            return self.remove_task_and_file(id).await;
        }

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
            self.schedule_tasks().await;
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
        let mut total_upload_speed: u64 = 0;
        let mut active = 0;
        let mut waiting = 0;
        let mut stopped = 0;

        for control in tasks.values() {
            match control.status.status.as_str() {
                "active" | "converting" => {
                    active += 1;
                    total_download_speed += self.calculate_current_speed(control);
                    if let Some(up_speed) = &control.status.upload_speed {
                        total_upload_speed += up_speed.parse::<u64>().unwrap_or(0);
                    }
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
            upload_speed: total_upload_speed.to_string(),
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
        self.schedule_tasks().await;
    }

    pub async fn get_global_option(&self) -> HashMap<String, String> {
        self.global_options.read().await.clone()
    }

    pub async fn change_option(
        self: &Arc<Self>,
        id: &str,
        options: HashMap<String, String>,
    ) -> bool {
        let mut change_seeding_to = None;
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
                    if k == "keep-seeding" {
                        let is_completed_torrent = control.status.file_type.as_deref() == Some("torrent") && {
                            let completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
                            let total = control.status.total_length.parse::<u64>().unwrap_or(0);
                            total > 0 && completed >= total
                        };
                        if is_completed_torrent {
                            change_seeding_to = Some(v == "true");
                        }
                    }
                    control.options.insert(k, v);
                }
                true
            } else {
                false
            }
        };

        if let Some(should_seed) = change_seeding_to {
            let handle = {
                let handles = self.torrent_handles.read().await;
                handles.get(id).cloned()
            };
            if let Some(handle) = handle {
                if let Ok(session) = self.get_torrent_session().await {
                    if should_seed {
                        if let Ok(_) = session.unpause(&handle).await {
                            let mut tasks = self.tasks.write().await;
                            if let Some(control) = tasks.get_mut(id) {
                                control.status.status = "active".to_string();
                                control.token = CancellationToken::new();
                                let token_clone = control.token.clone();
                                let manager_clone = self.clone();
                                let id_clone = id.to_string();
                                let handle_clone = handle.clone();
                                tokio::spawn(async move {
                                    manager_clone
                                        .run_torrent_stats_loop(id_clone, handle_clone, token_clone)
                                        .await;
                                });
                            }
                        }
                    } else {
                        let _ = session.pause(&handle).await;
                        let mut tasks = self.tasks.write().await;
                        if let Some(control) = tasks.get_mut(id) {
                            control.token.cancel();
                            control.status.status = "paused".to_string();
                        }
                    }
                }
            }
        }

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
        let reported_speed = control.status.download_speed.parse::<u64>().unwrap_or(0);
        if control.status.file_type == Some("torrent".to_string()) {
            return reported_speed;
        }

        if control.status.status != "active" && control.status.status != "converting" {
            return 0;
        }

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(control.last_update_time).as_secs_f64();

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
        let url_trimmed = url.trim().to_string();
        let url_lower = url_trimmed.to_lowercase();

        // A. Handle magnet link resolution via list_only
        if url_lower.starts_with("magnet:?") {
            let session = self.get_torrent_session().await?;
            let torrent_source = librqbit::AddTorrent::from_url(url_trimmed.clone());
            let opts = librqbit::AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };

            // Timeout magnet resolution after 15 seconds to prevent hanging the RPC connection
            let add_res_timeout = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                session.add_torrent(torrent_source, Some(opts)),
            )
            .await;

            let add_res = match add_res_timeout {
                Ok(res) => res.map_err(|e| format!("Failed to resolve magnet: {:?}", e))?,
                Err(_) => {
                    // Fallback to offline parsing on timeout
                    let parsed = librqbit::Magnet::parse(&url_trimmed)
                        .map_err(|e| format!("Invalid magnet URL: {:?}", e))?;
                    let name = parsed.name.clone();
                    return Ok(crate::models::ResolveResponse {
                        url: url_trimmed,
                        filename: name,
                        total_size: None,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: None,
                    });
                }
            };

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = self.parse_list_only_response(&res.info);
                    Ok(crate::models::ResolveResponse {
                        url: url_trimmed,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from magnet link resolver".to_string()),
            };
        }

        let global_opts = self.global_options.read().await;

        let mut builder = reqwest::Client::builder();

        if let Some(ua) = global_opts.get("user-agent").filter(|s| !s.is_empty()) {
            builder = builder.user_agent(ua);
        } else {
            builder = builder.user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        }

        if let Some(proxy_url) = global_opts.get("all-proxy").filter(|s| !s.is_empty()) {
            if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                builder = builder.proxy(proxy);
            }
        }

        let client = builder.build().map_err(|e| e.to_string())?;

        // B. Handle HTTP .torrent link resolution directly
        let is_direct_torrent = url_lower
            .split('?')
            .next()
            .unwrap_or("")
            .ends_with(".torrent");
        if is_direct_torrent {
            let response = client
                .get(&url_trimmed)
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let final_url = response.url().to_string();
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;

            let session = self.get_torrent_session().await?;
            let torrent_source = librqbit::AddTorrent::from_bytes(bytes.to_vec());
            let opts = librqbit::AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            let add_res = session
                .add_torrent(torrent_source, Some(opts))
                .await
                .map_err(|e| format!("Failed to resolve torrent link: {:?}", e))?;

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = self.parse_list_only_response(&res.info);
                    Ok(crate::models::ResolveResponse {
                        url: final_url,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from torrent link resolution".to_string()),
            };
        }

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

        // C. Handle standard response that redirects to a .torrent file
        let is_torrent = content_type.contains("application/x-bittorrent")
            || final_url
                .split('?')
                .next()
                .unwrap_or("")
                .ends_with(".torrent");

        if is_torrent {
            let bytes = response.bytes().await.map_err(|e| e.to_string())?;
            let session = self.get_torrent_session().await?;
            let torrent_source = librqbit::AddTorrent::from_bytes(bytes.to_vec());
            let opts = librqbit::AddTorrentOptions {
                list_only: true,
                ..Default::default()
            };
            let add_res = session
                .add_torrent(torrent_source, Some(opts))
                .await
                .map_err(|e| format!("Failed to resolve torrent link: {:?}", e))?;

            return match add_res {
                librqbit::AddTorrentResponse::ListOnly(res) => {
                    let (name, total_size, files) = self.parse_list_only_response(&res.info);
                    Ok(crate::models::ResolveResponse {
                        url: final_url,
                        filename: name,
                        total_size,
                        file_type: Some("torrent".to_string()),
                        is_resumable: Some(true),
                        torrent_files: Some(files),
                    })
                }
                _ => Err("Expected ListOnly response from torrent link resolver".to_string()),
            };
        }

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
                torrent_files: None,
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
                    torrent_files: None,
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
                    torrent_files: None,
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
                    torrent_files: None,
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
            torrent_files: None,
        })
    }

    fn parse_list_only_response(
        &self,
        info: &librqbit::TorrentMetaV1Info<librqbit::ByteBufOwned>,
    ) -> (
        Option<String>,
        Option<i64>,
        Vec<crate::models::TorrentResolveFile>,
    ) {
        let name = info
            .name
            .as_ref()
            .map(|n| String::from_utf8_lossy(n.as_ref()).into_owned());

        let mut files_resolved = Vec::new();
        let mut total_size: u64 = 0;

        if let Some(files) = &info.files {
            for (idx, f) in files.iter().enumerate() {
                let relative_path = f
                    .path
                    .iter()
                    .map(|p| String::from_utf8_lossy(p.as_ref()).into_owned())
                    .collect::<Vec<String>>()
                    .join("/");
                files_resolved.push(crate::models::TorrentResolveFile {
                    index: idx,
                    path: relative_path,
                    length: f.length,
                });
                total_size += f.length;
            }
        } else {
            // Single file mode
            let file_name = name.clone().unwrap_or_else(|| "download".to_string());
            let length = info.length.unwrap_or(0);
            files_resolved.push(crate::models::TorrentResolveFile {
                index: 0,
                path: file_name,
                length,
            });
            total_size = length;
        }

        (name, Some(total_size as i64), files_resolved)
    }

    pub async fn resolve_torrent_base64(
        &self,
        base64_str: String,
    ) -> Result<crate::models::ResolveResponse, String> {
        use base64::{engine::general_purpose, Engine as _};
        let decoded = general_purpose::STANDARD
            .decode(base64_str.trim())
            .map_err(|e| format!("Invalid base64: {:?}", e))?;

        let session = self.get_torrent_session().await?;
        let torrent_source = librqbit::AddTorrent::from_bytes(decoded);
        let opts = librqbit::AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };
        let add_res = session
            .add_torrent(torrent_source, Some(opts))
            .await
            .map_err(|e| format!("Failed to resolve torrent: {:?}", e))?;

        match add_res {
            librqbit::AddTorrentResponse::ListOnly(res) => {
                let (name, total_size, files) = self.parse_list_only_response(&res.info);
                Ok(crate::models::ResolveResponse {
                    url: "".to_string(),
                    filename: name,
                    total_size,
                    file_type: Some("torrent".to_string()),
                    is_resumable: Some(true),
                    torrent_files: Some(files),
                })
            }
            _ => Err("Expected ListOnly response from base64 torrent resolver".to_string()),
        }
    }

    pub async fn perform_format_conversion(
        &self,
        file_path_str: &str,
        url: &str,
        file_type: Option<&str>,
    ) -> Result<(), String> {
        let path = std::path::Path::new(file_path_str);
        if !path.exists() {
            return Ok(());
        }

        let target_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if target_ext.is_empty() {
            return Ok(());
        }

        // Try to find the source extension from URL
        let url_no_query = url.split('?').next().unwrap_or(url);
        let url_clean = url_no_query.split('#').next().unwrap_or(url_no_query);

        let mut src_ext = std::path::Path::new(url_clean)
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
            return Ok(());
        }

        println!(
            "[CONVERTER] Attempting conversion from {} to {}",
            src_ext, target_ext
        );

        let temp_path_str = format!("{}.src.{}", file_path_str, src_ext);
        let temp_path = std::path::Path::new(&temp_path_str);

        // Rename the downloaded file to a temp file containing the raw source bytes
        if let Err(e) = std::fs::rename(path, temp_path) {
            let msg = format!(
                "[CONVERTER ERROR] Failed to rename original file to temp path: {}",
                e
            );
            eprintln!("{}", msg);
            return Err(msg);
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
            Ok(())
        } else {
            // Restore original file so no data is lost
            let _ = std::fs::rename(temp_path, path);
            let msg = format!(
                "Conversion failed or unsupported. Restored original file as {}.",
                path.display()
            );
            eprintln!("[CONVERTER] {}", msg);
            Ok(())
        }
    }
}
