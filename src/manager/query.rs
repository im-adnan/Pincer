//! Status queries (get_task, get_active, get_waiting, get_stopped, get_global_stat)
//!
//! ### Architectural Overview
//! - **What it does**: Queries the task registry to return individual task status, active tasks, waiting queues, stopped results, and global aggregate bandwidth speeds.
//! - **How it does**: Reads `tasks` read locks, filters task status variants, computes smoothed speeds via `SpeedCalculator::calculate_speed()`, and tracks historical peak download bandwidth.
//! - **Where it comes from**: Called by `manager.get_task()`, `manager.get_active_tasks()`, `manager.get_waiting_tasks()`, `manager.get_stopped_tasks()`, and `manager.get_global_stat()`.
//! - **Where it leads to**: Returns structured `TaskStatus` and `GlobalStat` vectors to RPC query handlers.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::speed::SpeedCalculator;
use super::state::TaskControl;
use crate::models::{GlobalStat, TaskStatus};

/// Handles task registry read queries and aggregate statistics calculation.
pub struct TaskQueryManager;

impl TaskQueryManager {
    /// Queries status for a specific task GID, calculating its real-time speed.
    pub async fn get_task(
        id: &str,
        tasks: &RwLock<HashMap<String, TaskControl>>,
    ) -> Option<TaskStatus> {
        let tasks_guard = tasks.read().await;
        tasks_guard.get(id).map(|c| {
            let mut status = c.status.clone();
            status.download_speed = SpeedCalculator::calculate_speed(c).to_string();
            status
        })
    }

    /// Queries all currently downloading or converting tasks.
    pub async fn get_active(tasks: &RwLock<HashMap<String, TaskControl>>) -> Vec<TaskStatus> {
        let tasks_guard = tasks.read().await;
        tasks_guard
            .values()
            .filter(|c| c.status.status == "active" || c.status.status == "converting")
            .map(|c| {
                let mut status = c.status.clone();
                status.download_speed = SpeedCalculator::calculate_speed(c).to_string();
                status
            })
            .collect()
    }

    /// Queries waiting and paused tasks.
    pub async fn get_waiting(tasks: &RwLock<HashMap<String, TaskControl>>) -> Vec<TaskStatus> {
        let tasks_guard = tasks.read().await;
        tasks_guard
            .values()
            .filter(|c| c.status.status == "waiting" || c.status.status == "paused")
            .map(|c| c.status.clone())
            .collect()
    }

    /// Queries completed, failed, or removed tasks.
    pub async fn get_stopped(tasks: &RwLock<HashMap<String, TaskControl>>) -> Vec<TaskStatus> {
        let tasks_guard = tasks.read().await;
        tasks_guard
            .values()
            .filter(|c| {
                c.status.status == "complete"
                    || c.status.status == "error"
                    || c.status.status == "removed"
            })
            .map(|c| c.status.clone())
            .collect()
    }

    /// Computes aggregate global metrics across all registered tasks.
    ///
    /// Metric aggregation:
    /// 1. Sums download speed across all active/converting tasks.
    /// 2. Sums upload speed across all active BitTorrent seeders.
    /// 3. Categorizes tasks into active, waiting, and stopped buckets.
    /// 4. Updates peak seen download bandwidth (`max_seen_speed`) for adaptive modes.
    pub async fn get_global_stat(
        tasks: &RwLock<HashMap<String, TaskControl>>,
        global_options: &RwLock<HashMap<String, String>>,
        max_seen_speed: &Arc<AtomicU64>,
    ) -> GlobalStat {
        let tasks_guard = tasks.read().await;
        let mut total_download_speed: u64 = 0;
        let mut total_upload_speed: u64 = 0;
        let mut active = 0;
        let mut waiting = 0;
        let mut stopped = 0;

        for control in tasks_guard.values() {
            match control.status.status.as_str() {
                "active" | "converting" => {
                    active += 1;
                    total_download_speed += SpeedCalculator::calculate_speed(control);
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
            let opts = global_options.read().await;
            opts.get("speed-mode")
                .cloned()
                .unwrap_or_else(|| "max_bandwidth".to_string())
        };

        if (current_mode == "max_bandwidth" || current_mode == "max")
            && total_download_speed > max_seen_speed.load(Ordering::Relaxed)
        {
            max_seen_speed.store(total_download_speed, Ordering::Relaxed);
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
}
