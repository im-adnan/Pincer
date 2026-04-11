use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tokio_util::sync::CancellationToken;

use crate::models::{TaskStatus, GlobalStat, NotificationParam, RPCNotification, FileData, FileUri};

struct TaskControl {
    status: TaskStatus,
    token: CancellationToken,
    options: HashMap<String, String>,
}

pub struct DownloadManager {
    tasks: RwLock<HashMap<String, TaskControl>>,
    global_options: RwLock<HashMap<String, String>>,
    tx: broadcast::Sender<String>,
}

impl DownloadManager {
    pub fn new() -> (Arc<Self>, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(100);
        let manager = Arc::new(Self {
            tasks: RwLock::new(HashMap::new()),
            global_options: RwLock::new(HashMap::new()),
            tx,
        });
        (manager, rx)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.tx.subscribe()
    }

    pub async fn _add_task(&self, id: String, status: TaskStatus, token: CancellationToken, options: HashMap<String, String>) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), TaskControl { status, token, options });
        // Dispatch start event
        let _ = self.tx.send(self.build_notification("pin.onDownloadStart", &id));
    }

    pub async fn update_task_progress(&self, id: &str, downloaded_chunk: u64) {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
            let current_completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
            control.status.completed_length = (current_completed + downloaded_chunk).to_string();
        }
    }

    pub async fn spawn_task(self: &Arc<Self>, id: String, url: String, filename: String, dir: String, threads: usize, resume_offset: u64) {
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
            
            tasks.insert(id.clone(), TaskControl { 
                status: initial_status, 
                token: token.clone(),
                options: opts,
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
        let (url, filename, dir, resume_offset, threads) = {
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
                (url, filename, dir, resume_offset, threads)
            } else {
                return false;
            }
        };

        self.spawn_task(id.to_string(), url, filename, dir, threads, resume_offset).await;
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
        tasks.get(id).map(|c| c.status.clone())
    }

    pub async fn get_active_tasks(&self) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|c| c.status.status == "active").map(|c| c.status.clone()).collect()
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
        let active = self.get_active_tasks().await.len();
        let waiting = self.get_waiting_tasks(0, 0).await.len();
        let stopped = self.get_stopped_tasks(0, 0).await.len();

        GlobalStat {
            download_speed: "0".to_string(),
            upload_speed: "0".to_string(),
            num_active: active.to_string(),
            num_waiting: waiting.to_string(),
            num_stopped: stopped.to_string(),
            num_stopped_total: stopped.to_string(),
        }
    }

    pub async fn change_global_option(&self, options: HashMap<String, String>) {
        let mut global_opts = self.global_options.write().await;
        for (k, v) in options {
            global_opts.insert(k, v);
        }
    }

    pub async fn get_global_option(&self) -> HashMap<String, String> {
        self.global_options.read().await.clone()
    }

    pub async fn change_option(&self, id: &str, options: HashMap<String, String>) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(control) = tasks.get_mut(id) {
            for (k, v) in options {
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
}
