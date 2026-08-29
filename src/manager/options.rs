//! Option & GlobalOption getters/setters & speed modes
//!
//! ### Architectural Overview
//! - **What it does**: Manages global and per-task options, configuring bandwidth speed profiles (`max_bandwidth`, `half_bandwidth`, `min_bandwidth`), download directories, split worker counts, and seeding policies.
//! - **How it does**: Mutates `global_options` map and updates `current_limit`, `default_split`, and `max_seen_speed` atomic values with relaxed atomic orderings.
//! - **Where it comes from**: Called by `manager.change_global_option()` and `manager.change_option()`.
//! - **Where it leads to**: Affects download concurrency, rate limiting in `RateThrottler`, and persists updated settings to session storage.

use super::state::TaskControl;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages runtime option changes and bandwidth speed mode profiles.
pub struct OptionsManager;

impl OptionsManager {
    /// Applies global configuration option updates.
    ///
    /// Handles adaptive bandwidth speed modes:
    /// - `"max_bandwidth"` / `"max"`: Sets limit to 0 (unlimited).
    /// - `"half_bandwidth"` / `"half"`: Sets limit to 50% of peak observed bandwidth (`max_seen_speed / 2`).
    /// - `"min_bandwidth"` / `"min"`: Sets limit to minimal trickle speed (768 B/s).
    pub async fn apply_global_options(
        options: HashMap<String, String>,
        global_options: &RwLock<HashMap<String, String>>,
        current_limit: &Arc<AtomicU64>,
        max_seen_speed: &Arc<AtomicU64>,
        default_split: &Arc<AtomicU64>,
    ) {
        if let Some(mode) = options.get("speed-mode") {
            match mode.as_str() {
                "max_bandwidth" | "max" => {
                    current_limit.store(0, Ordering::Relaxed);
                }
                "half_bandwidth" | "half" => {
                    let peak = max_seen_speed.load(Ordering::Relaxed);
                    let half_speed = std::cmp::max(peak / 2, 1024 * 1024);
                    current_limit.store(half_speed, Ordering::Relaxed);
                }
                "min_bandwidth" | "min" => {
                    current_limit.store(768, Ordering::Relaxed);
                }
                _ => {}
            }
        }

        if let Some(limit_str) = options.get("max-overall-download-limit") {
            if let Ok(limit) = limit_str.parse::<u64>() {
                current_limit.store(limit, Ordering::Relaxed);
            }
        }

        if let Some(split_str) = options
            .get("split")
            .or_else(|| options.get("default-split"))
        {
            if let Ok(split) = split_str.parse::<u64>() {
                default_split.store(split.min(99), Ordering::Relaxed);
            }
        }

        let mut global_opts = global_options.write().await;
        for (k, v) in options {
            global_opts.insert(k, v);
        }
    }

    /// Mutates configuration options for a specific task GID.
    pub async fn apply_task_options(
        id: &str,
        options: HashMap<String, String>,
        tasks: &RwLock<HashMap<String, TaskControl>>,
    ) -> (bool, Option<bool>) {
        let mut change_seeding_to = None;
        let mut tasks_guard = tasks.write().await;

        if let Some(control) = tasks_guard.get_mut(id) {
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
                    let is_completed_torrent =
                        control.status.file_type.as_deref() == Some("torrent") && {
                            let completed =
                                control.status.completed_length.parse::<u64>().unwrap_or(0);
                            let total = control.status.total_length.parse::<u64>().unwrap_or(0);
                            total > 0 && completed >= total
                        };
                    if is_completed_torrent {
                        change_seeding_to = Some(v == "true");
                    }
                }
                control.options.insert(k, v);
            }
            (true, change_seeding_to)
        } else {
            (false, None)
        }
    }
}
