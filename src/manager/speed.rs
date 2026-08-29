//! Speed calculation & smoothing algorithms
//!
//! ### Architectural Overview
//! - **What it does**: Calculates real-time instantaneous and smoothed download transfer rates for active tasks.
//! - **How it does**: Compares progress deltas against elapsed wall-clock time intervals; if recent updates exist within 1.0s, returns worker-reported speed; otherwise decays speed over elapsed time.
//! - **Where it comes from**: Called by `manager::TaskQueryManager` when generating task statuses and computing global bandwidth statistics.
//! - **Where it leads to**: Returns byte-per-second transfer rates formatted in `TaskStatus` and `GlobalStat` responses.

use super::state::TaskControl;
use std::time::Instant;

/// Calculates real-time instantaneous and decaying bandwidth transfer rates.
pub struct SpeedCalculator;

impl SpeedCalculator {
    /// Computes instantaneous download speed (bytes/second) for a given task.
    ///
    /// Speed calculation mechanics:
    /// 1. BitTorrent tasks: Returns the native speed reported by `librqbit`.
    /// 2. Inactive tasks: Returns 0.
    /// 3. If progress was updated recently (<= 1.0s), returns worker-reported speed.
    /// 4. If progress has stalled, computes decaying speed over elapsed wall-clock time.
    pub fn calculate_speed(control: &TaskControl) -> u64 {
        let reported_speed = control.status.download_speed.parse::<u64>().unwrap_or(0);
        if control.status.file_type == Some("torrent".to_string()) {
            return reported_speed;
        }

        if control.status.status != "active" && control.status.status != "converting" {
            return 0;
        }

        let now = Instant::now();
        let elapsed = now.duration_since(control.last_update_time).as_secs_f64();

        // If updated recently (within 1s), return the worker-reported speed
        if elapsed <= 1.0 {
            return reported_speed;
        }

        // Otherwise calculate decaying speed over wall-clock delta
        let completed = control.status.completed_length.parse::<u64>().unwrap_or(0);
        let bytes_diff = completed.saturating_sub(control.last_update_bytes);
        (bytes_diff as f64 / elapsed) as u64
    }
}
