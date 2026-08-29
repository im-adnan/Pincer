//! Bandwidth throttling & ThreadGuard RAII
//!
//! ### Architectural Overview
//! - **What it does**: Enforces global bandwidth speed limits across concurrent download workers and tracks active thread counts using RAII guards.
//! - **How it does**: Divides global bandwidth quota by currently active threads, calculates sleep intervals when transferred chunk rates exceed per-thread quotas, and decrements active threads via `ThreadGuard::drop()`.
//! - **Where it comes from**: Called by `engine::DownloadWorker` inside the byte streaming loop.
//! - **Where it leads to**: Inserts async `tokio::time::sleep()` pauses to shape download throughput without packet dropping.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration, Instant};

/// RAII guard that automatically increments `active_threads` on creation and decrements on drop.
pub struct ThreadGuard {
    active_threads: Arc<AtomicU64>,
}

impl ThreadGuard {
    /// Creates a new `ThreadGuard`, atomically incrementing the active thread counter.
    pub fn new(active_threads: Arc<AtomicU64>) -> Self {
        active_threads.fetch_add(1, Ordering::Relaxed);
        Self { active_threads }
    }
}

impl Drop for ThreadGuard {
    /// Decrements the active thread counter when the worker thread finishes or terminates.
    fn drop(&mut self) {
        self.active_threads.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Enforces proportional bandwidth rate limiting across all active download workers.
pub struct RateThrottler;

impl RateThrottler {
    /// Enforces per-worker bandwidth rate limits based on global bandwidth quota.
    ///
    /// Algorithmic breakdown:
    /// 1. Reads global limit and active thread count atomically.
    /// 2. If global limit is 0 (unlimited) or chunk is 0 bytes, returns immediately.
    /// 3. Computes per-thread share: `thread_limit = global_limit / active_threads`.
    /// 4. Calculates target transmission duration for `chunk_size` bytes.
    /// 5. If elapsed transfer time is shorter than expected, sleeps for the remainder duration.
    pub async fn throttle(
        chunk_size: usize,
        last_chunk_time: &mut Instant,
        global_limit: &Arc<AtomicU64>,
        active_threads: &Arc<AtomicU64>,
    ) {
        let limit = global_limit.load(Ordering::Relaxed);
        if limit == 0 || chunk_size == 0 {
            *last_chunk_time = Instant::now();
            return;
        }

        let threads = active_threads.load(Ordering::Relaxed).max(1);
        let thread_limit = (limit / threads).max(1024); // Floor at 1 KB/s per thread

        let expected_duration_secs = chunk_size as f64 / thread_limit as f64;
        let expected_duration = Duration::from_secs_f64(expected_duration_secs);

        let elapsed = last_chunk_time.elapsed();
        if elapsed < expected_duration {
            let delay = expected_duration - elapsed;
            sleep(delay).await;
        }

        *last_chunk_time = Instant::now();
    }
}
