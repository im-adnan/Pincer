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
