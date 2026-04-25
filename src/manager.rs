use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;
use regex::Regex;
use serde_json::Value;

use crate::models::{TaskStatus, GlobalStat, NotificationParam, RPCNotification, FileData, FileUri, ResolveResponse};

struct TaskControl {
    status: TaskStatus,
    token: CancellationToken,
    options: HashMap<String, String>,
    last_update_bytes: u64,
    last_update_time: std::time::Instant,
}

pub struct DownloadManager {
    tasks: RwLock<HashMap<String, TaskControl>>,
    global_options: RwLock<HashMap<String, String>>,
    tx: broadcast::Sender<String>,
    pub current_limit: Arc<AtomicU64>,
    pub max_seen_speed: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
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
        });
        (manager, rx)
    }

    pub async fn generate_unique_filename(&self, filename: &str, excluding_gid: Option<&str>) -> String {
        let tasks = self.tasks.read().await;
        let mut unique_name = filename.to_string();
        let mut counter = 1;
        
        let path = std::path::Path::new(filename);
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or(filename);
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        while tasks.values().any(|c| {
            if let Some(egid) = excluding_gid {
                if c.status.gid == egid { return false; }
            }
            if let Some(file) = c.status.files.first() {
                let existing_filename = std::path::Path::new(&file.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if existing_filename == unique_name { return true; }
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

    pub async fn _add_task(&self, id: String, status: TaskStatus, token: CancellationToken, options: HashMap<String, String>) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), TaskControl { 
            status, 
            token, 
            options,
            last_update_bytes: 0,
            last_update_time: std::time::Instant::now(),
        });
        // Dispatch start event
        let _ = self.tx.send(self.build_notification("pin.onDownloadStart", &id));
    }

    pub async fn update_task_progress(&self, id: &str, downloaded_chunk: u64) {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
            let current_completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
            let new_completed = current_completed + downloaded_chunk;
            control.status.completed_length = new_completed.to_string();
            
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

    pub async fn spawn_task(self: &Arc<Self>, id: String, url: String, mut filename: String, dir: String, threads: usize, resume_offset: u64, headers: Vec<String>) {
        filename = self.generate_unique_filename(&filename, None).await;
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
            
            tasks.insert(id.clone(), TaskControl { 
                status: initial_status, 
                token: token.clone(),
                options: opts,
                last_update_bytes: 0,
                last_update_time: std::time::Instant::now(),
            });
        }
        
        let _ = self.tx.send(self.build_notification("pin.onDownloadStart", &id));

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
                Ok((total_size, mut progress_rx)) => {
                    {
                        let mut locks = manager_clone.tasks.write().await;
                        if let Some(control) = locks.get_mut(&id_clone) {
                            control.status.total_length = total_size.to_string();
                        }
                    }
                    
                    while let Some((_worker_id, bytes_chunk)) = progress_rx.recv().await {
                        manager_clone.update_task_progress(&id_clone, bytes_chunk).await;
                    }
                    
                    // Check if it was cancelled or finished
                    let mut locks = manager_clone.tasks.write().await;
                    if let Some(control) = locks.get_mut(&id_clone) {
                        if token.is_cancelled() {
                            control.status.status = "paused".to_string();
                            let _ = manager_clone.tx.send(manager_clone.build_notification("pin.onDownloadPause", &id_clone));
                        } else {
                            control.status.status = "complete".to_string();
                            let _ = manager_clone.tx.send(manager_clone.build_notification("pin.onDownloadComplete", &id_clone));
                        }
                    }
                },
                Err(e) => {
                    eprintln!("Task '{}' failed: {}", id_clone, e);
                    let mut locks = manager_clone.tasks.write().await;
                    if let Some(control) = locks.get_mut(&id_clone) {
                        control.status.status = "error".to_string();
                    }
                    let _ = manager_clone.tx.send(manager_clone.build_notification("pin.onDownloadError", &id_clone));
                }
            }
        });
    }

    pub async fn pause_task(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
            control.token.cancel();
            control.status.status = "paused".to_string();
            // Dispatch pause event immediately for UI responsiveness
            let _ = self.tx.send(self.build_notification("pin.onDownloadPause", id));
            true
        } else {
            false
        }
    }

    pub async fn pause_all_tasks(&self) {
        let mut tasks = self.tasks.write().await;
        for (gid, control) in tasks.iter_mut() {
            if control.status.status == "active" || control.status.status == "waiting" {
                control.token.cancel();
                control.status.status = "paused".to_string();
                let _ = self.tx.send(self.build_notification("pin.onDownloadPause", gid));
            }
        }
    }

    pub async fn unpause_task(self: &Arc<Self>, id: &str) -> bool {
        let (url, filename, dir, resume_offset, threads, headers) = {
            let tasks = self.tasks.read().await;
            if let Some(control) = tasks.get(id) {
                // If it's already active, don't start it again
                // Unless its token was cancelled (meaning it's in the process of stopping)
                if control.status.status == "active" && !control.token.is_cancelled() {
                    return false;
                }
                if control.status.status == "complete" {
                    return false;
                }
                let url = control.status.files[0].uris[0].uri.clone();
                let filename = std::path::Path::new(&control.status.files[0].path)
                    .file_name().unwrap_or_default().to_string_lossy().to_string();
                let dir = control.status.dir.clone();
                let resume_offset = control.status.completed_length.parse::<u64>().unwrap_or(0);
                let threads = control.options.get("split").and_then(|s| s.parse::<usize>().ok()).unwrap_or(4);
                let headers: Vec<String> = if let Some(header_str) = control.options.get("header") {
                    header_str.split('\n').filter(|s| !s.trim().is_empty()).map(|s| s.trim().to_string()).collect()
                } else {
                    Vec::new()
                };
                (url, filename, dir, resume_offset, threads, headers)
            } else {
                return false;
            }
        };

        self.spawn_task(id.to_string(), url, filename, dir, threads, resume_offset, headers).await;
        true
    }

    pub async fn unpause_all_tasks(self: &Arc<Self>) {
        let gids: Vec<String> = {
            let tasks = self.tasks.read().await;
            tasks.iter()
                .filter(|(_, c)| {
                    c.status.status == "paused" || 
                    c.status.status == "waiting" || 
                    c.status.status == "error" || 
                    c.status.status == "removed"
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        for gid in gids {
            self.unpause_task(&gid).await;
        }
    }

    pub async fn remove_task(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.remove(id) {
            control.token.cancel();
            true
        } else {
            false
        }
    }

    pub async fn remove_task_and_file(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.remove(id) {
            control.token.cancel();
            
            // Attempt to move files to trash
            for file in &control.status.files {
                let path = std::path::Path::new(&file.path);
                if path.exists() {
                    if let Err(e) = trash::delete(path) {
                        eprintln!("[ERROR] Failed to move file to trash '{}': {}", file.path, e);
                    } else {
                        println!("[INFO] Moved to trash: {}", file.path);
                    }
                }
            }
            true
        } else {
            false
        }
    }

    pub async fn force_remove_task(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.remove(id) {
            control.token.cancel();
            true
        } else {
            false
        }
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
        tasks.values()
            .filter(|c| c.status.status == "active")
            .map(|c| {
                let mut status = c.status.clone();
                status.download_speed = self.calculate_current_speed(c).to_string();
                status
            })
            .collect()
    }

    pub async fn get_waiting_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|c| c.status.status == "waiting" || c.status.status == "paused").map(|c| c.status.clone()).collect()
    }

    pub async fn get_stopped_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|c| c.status.status == "complete" || c.status.status == "error" || c.status.status == "removed").map(|c| c.status.clone()).collect()
    }

    pub async fn get_global_stat(&self) -> GlobalStat {
        let tasks = self.tasks.read().await;
        let mut total_download_speed: u64 = 0;
        let mut active = 0;
        let mut waiting = 0;
        let mut stopped = 0;

        for control in tasks.values() {
            match control.status.status.as_str() {
                "active" => {
                    active += 1;
                    total_download_speed += self.calculate_current_speed(control);
                },
                "waiting" | "paused" => waiting += 1,
                "complete" | "error" | "removed" => stopped += 1,
                _ => {}
            }
        }

        let current_mode = {
            let opts = self.global_options.read().await;
            opts.get("speed-mode").cloned().unwrap_or_else(|| "max_bandwidth".to_string())
        };

        if (current_mode == "max_bandwidth" || current_mode == "max") && total_download_speed > self.max_seen_speed.load(Ordering::Relaxed) {
             self.max_seen_speed.store(total_download_speed, Ordering::Relaxed);
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
                },
                "half_bandwidth" | "half" => {
                    let peak = self.max_seen_speed.load(Ordering::Relaxed);
                    // Use peak / 2, with a fallback floor of 1MB/s if peak is unknown
                    let half_speed = std::cmp::max(peak / 2, 1024 * 1024);
                    self.current_limit.store(half_speed, Ordering::Relaxed);
                },
                "min_bandwidth" | "min" => {
                    // Refined min: Sub-kb/s limit, NO pause
                    self.current_limit.store(768, Ordering::Relaxed); // 768 bytes/s is sub-kb
                },
                _ => {}
            }
        }

        if let Some(limit_str) = options.get("max-overall-download-limit") {
            if let Ok(limit) = limit_str.parse::<u64>() {
                self.current_limit.store(limit, Ordering::Relaxed);
            }
        }

        let mut global_opts = self.global_options.write().await;
        for (k, v) in options {
            global_opts.insert(k.clone(), v.clone());
        }
    }

    pub async fn get_global_option(&self) -> HashMap<String, String> {
        self.global_options.read().await.clone()
    }

    pub async fn change_option(&self, id: &str, options: HashMap<String, String>) -> bool {
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
    }

    pub async fn get_option(&self, id: &str) -> Option<HashMap<String, String>> {
        let tasks = self.tasks.read().await;
        tasks.get(id).map(|c| c.options.clone())
    }

    pub async fn purge_download_result(&self) {
        let mut tasks = self.tasks.write().await;
        tasks.retain(|_, control| {
            control.status.status != "complete" && 
            control.status.status != "error" && 
            control.status.status != "removed"
        });
    }

    pub async fn remove_download_result(&self, id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get(id) {
            if control.status.status == "complete" || control.status.status == "error" || control.status.status == "removed" {
                tasks.remove(id);
                return true;
            }
        }
        false
    }

    fn calculate_current_speed(&self, control: &TaskControl) -> u64 {
        if control.status.status != "active" {
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

    pub async fn resolve_url(&self, url: String) -> Result<ResolveResponse, String> {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| e.to_string())?;

        // 1. Standard Redirect Resolution
        let response = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let final_url = response.url().to_string();
        
        let headers = response.headers();
        let content_type = headers.get(reqwest::header::CONTENT_TYPE).and_then(|h| h.to_str().ok()).unwrap_or("");
        let content_length = headers.get(reqwest::header::CONTENT_LENGTH).and_then(|h| h.to_str().ok()).and_then(|s| s.parse::<u64>().ok());
        let accept_ranges = headers.get(reqwest::header::ACCEPT_RANGES).and_then(|h| h.to_str().ok()).unwrap_or("");
        let content_range = headers.get(reqwest::header::CONTENT_RANGE).and_then(|h| h.to_str().ok()).unwrap_or("");
        
        let is_resumable = accept_ranges.contains("bytes") || !content_range.is_empty();
        
        let file_type = if content_type.contains("video/") {
            Some("Video File".to_string())
        } else if content_type.contains("audio/") {
            Some("Audio File".to_string())
        } else if content_type.contains("image/") {
            Some("Image".to_string())
        } else if content_type.contains("application/pdf") {
            Some("PDF Document".to_string())
        } else if content_type.contains("zip") || content_type.contains("archive") {
            Some("Archive".to_string())
        } else {
            None
        };

        // Filename from Content-Disposition
        let mut filename = headers.get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|h| h.to_str().ok())
            .and_then(|s| {
                if let Some(idx) = s.find("filename=") {
                    let part = &s[idx + 9..];
                    let name = part.trim_matches('"');
                    Some(name.to_string())
                } else if let Some(idx) = s.find("filename*=") {
                    let part = &s[idx + 11..];
                    let val = part.split("''").last()?;
                    percent_encoding::percent_decode_str(val).decode_utf8().ok().map(|s: std::borrow::Cow<str>| s.to_string())
                } else {
                    None
                }
            });

        // 2. UNIVERSAL FALLBACK: Scrape HTML for metadata if it's an HTML page
        if content_type.contains("text/html") && (filename.is_none() || !content_type.contains("video/")) {
            let html = response.text().await.map_err(|e| e.to_string())?;

            // A. Universal Meta Tags
            let og_video_re = Regex::new(r#"<meta property="(?:og:video|twitter:player)" content="(.*?)"#).unwrap();
            let og_title_re = Regex::new(r#"<meta property="(?:og:title|twitter:title)" content="(.*?)"#).unwrap();

            if let Some(caps) = og_video_re.captures(&html) {
                let video_url = caps[1].to_string();
                let title = og_title_re.captures(&html).map(|c| c[1].to_string()).unwrap_or_else(|| "download".to_string());
                return Ok(ResolveResponse {
                    url: video_url,
                    filename: Some(format!("{}.mp4", title.replace(" ", "-"))),
                    total_size: None,
                    file_type: Some("Video File".to_string()),
                    is_resumable: Some(true),
                });
            }

            // B. Universal JSON Script Extraction
            let json_re = Regex::new(r#"<script[^>]*type="application/json"[^>]*>(.*?)</script>|<script id="__NEXT_DATA__"[^>]*>(.*?)</script>"#).unwrap();
            let video_link_re = Regex::new(r#"https?://[^\s"\'<>]+?\.(?:mp4|mkv|webm|mov)(?:[^\s"\'<>]*?)"#).unwrap();
            
            for caps in json_re.captures_iter(&html) {
                let json_content = caps.get(1).or(caps.get(2)).map(|m| m.as_str()).unwrap_or("");
                if let Ok(data) = serde_json::from_str::<Value>(json_content) {
                    let json_str = data.to_string();
                    if let Some(mat) = video_link_re.find(&json_str) {
                        let link = mat.as_str().to_string();
                        let title = og_title_re.captures(&html).map(|c| c[1].to_string()).unwrap_or_else(|| "download".to_string());
                        let ext = link.split('?').next().unwrap_or("").split('.').last().unwrap_or("mp4").to_string();
                        return Ok(ResolveResponse {
                            url: link,
                            filename: Some(format!("{}.{}", title.replace(" ", "-"), ext)),
                            total_size: None,
                            file_type: Some("Video File".to_string()),
                            is_resumable: Some(true),
                        });
                    }
                }
            }
        }

        if filename.is_none() {
            filename = Some(final_url.split('/').last().unwrap_or("download.bin").split('?').next().unwrap_or("download.bin").to_string());
        }

        Ok(ResolveResponse {
            url: final_url,
            filename,
            total_size: content_length,
            file_type,
            is_resumable: Some(is_resumable),
        })
    }
}
