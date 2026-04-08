use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

use crate::models::{TaskStatus, GlobalStat, NotificationParam, RPCNotification};

pub struct DownloadManager {
    tasks: RwLock<HashMap<String, TaskStatus>>,
    tx: broadcast::Sender<String>,
}

impl DownloadManager {
    pub fn new() -> (Arc<Self>, broadcast::Receiver<String>) {
        let (tx, rx) = broadcast::channel(100);
        let manager = Arc::new(Self {
            tasks: RwLock::new(HashMap::new()),
            tx,
        });
        (manager, rx)
    }

    pub async fn add_task(&self, id: String, status: TaskStatus) {
        let mut tasks = self.tasks.write().await;
        tasks.insert(id.clone(), status);
        // Dispatch start event
        let _ = self.tx.send(self.build_notification("pin.onDownloadStart", &id));
    }

    pub async fn update_task_progress(&self, id: &str, downloaded_chunk: u64) {
        let mut tasks = self.tasks.write().await;
        if let Some(task) = tasks.get_mut(id) {
            let current_completed = task.completed_length.parse::<u64>().unwrap_or(0);
            task.completed_length = (current_completed + downloaded_chunk).to_string();
            // Optional: compute instantaneous speed here
        }
    }

    pub async fn spawn_task(self: &Arc<Self>, id: String, url: String, filename: String, dir: String, threads: usize) {
        use crate::models::{FileData, FileUri};
        
        // 1. Register the task initially as "waiting" or "active"
        let initial_status = TaskStatus {
            gid: id.clone(),
            status: "active".to_string(), // starting immediately
            total_length: "0".to_string(),
            completed_length: "0".to_string(),
            download_speed: "0".to_string(),
            files: vec![FileData {
                path: format!("{}/{}", dir, filename),
                uris: vec![FileUri { uri: url.clone() }],
            }],
            dir: dir.clone(),
        };
        
        self.add_task(id.clone(), initial_status).await;

        let manager_clone = self.clone();
        
        // 2. Spawn the background rust task to do the downloading
        tokio::spawn(async move {
            let task = crate::task::DownloadTask {
                id: id.clone(),
                url: url.clone(),
                filename: filename.clone(),
                save_path: dir.clone(),
                threads,
                total_size: 0,
            };

            match task.start().await {
                Ok((total_size, mut progress_rx)) => {
                    // Update initial total length immediately
                    {
                        let mut locks = manager_clone.tasks.write().await;
                        if let Some(t) = locks.get_mut(&id) {
                            t.total_length = total_size.to_string();
                        }
                    }
                    
                    while let Some((_worker_id, bytes_chunk)) = progress_rx.recv().await {
                        manager_clone.update_task_progress(&id, bytes_chunk).await;
                    }
                    
                    // When progress_rx closes, workers are done!
                    let mut locks = manager_clone.tasks.write().await;
                    if let Some(t) = locks.get_mut(&id) {
                        t.status = "complete".to_string();
                    }
                    let _ = manager_clone.tx.send(manager_clone.build_notification("pin.onDownloadComplete", &id));
                },
                Err(e) => {
                    eprintln!("Task '{}' failed: {}", id, e);
                    let mut locks = manager_clone.tasks.write().await;
                    if let Some(t) = locks.get_mut(&id) {
                        t.status = "error".to_string();
                    }
                    let _ = manager_clone.tx.send(manager_clone.build_notification("pin.onDownloadError", &id));
                }
            }
        });
    }

    pub async fn get_task(&self, id: &str) -> Option<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.get(id).cloned()
    }

    pub async fn get_active_tasks(&self) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|t| t.status == "active").cloned().collect()
    }

    pub async fn get_waiting_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|t| t.status == "waiting" || t.status == "paused").cloned().collect()
    }

    pub async fn get_stopped_tasks(&self, _offset: usize, _num: usize) -> Vec<TaskStatus> {
        let tasks = self.tasks.read().await;
        tasks.values().filter(|t| t.status == "complete" || t.status == "error" || t.status == "removed").cloned().collect()
    }

    pub async fn get_global_stat(&self) -> GlobalStat {
        GlobalStat {
            download_speed: "0".to_string(), // TODO: calculate total speed
            upload_speed: "0".to_string(),
            num_active: self.get_active_tasks().await.len().to_string(),
            num_waiting: "0".to_string(),
            num_stopped: "0".to_string(),
            num_stopped_total: "0".to_string(),
        }
    }

    // Helpers to broadcast events
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
