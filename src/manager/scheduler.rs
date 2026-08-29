//! Queue scheduling with max-concurrent-downloads
//!
//! ### Architectural Overview
//! - **What it does**: Inspects the task queue and selects eligible waiting tasks to run while strictly respecting the `max-concurrent-downloads` limit.
//! - **How it does**: Counts currently active/converting tasks, sorts waiting tasks in FIFO order by `created_at` timestamp, and returns a list of GIDs eligible to start.
//! - **Where it comes from**: Called by `manager.schedule_tasks()` on task creation, unpausing, completion, or option modifications.
//! - **Where it leads to**: Hands off selected task GIDs to `manager.execute_task()` to start downloading.

use super::state::TaskControl;
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Evaluates waiting task queues and determines which tasks should begin execution.
pub struct TaskScheduler;

impl TaskScheduler {
    /// Selects next waiting task GIDs eligible for execution based on `max-concurrent-downloads`.
    ///
    /// Scheduling algorithm:
    /// 1. Reads `max-concurrent-downloads` option (default: 5 concurrent tasks).
    /// 2. Iterates through all registered tasks, counting active/converting tasks.
    /// 3. Collects waiting tasks and sorts them in FIFO order by creation timestamp (`created_at`).
    /// 4. Fills available concurrency slots and returns the list of runnable task GIDs.
    pub async fn select_runnable_tasks(
        tasks: &RwLock<HashMap<String, TaskControl>>,
        global_options: &RwLock<HashMap<String, String>>,
    ) -> Vec<String> {
        let max_concurrent = {
            let opts = global_options.read().await;
            opts.get("max-concurrent-downloads")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(5)
        };

        let mut active_count = 0;
        let mut waiting_tasks = Vec::new();

        {
            let tasks_guard = tasks.read().await;
            for (gid, control) in tasks_guard.iter() {
                if control.status.status == "active" || control.status.status == "converting" {
                    active_count += 1;
                } else if control.status.status == "waiting" {
                    waiting_tasks.push((gid.clone(), control.created_at));
                }
            }
        }

        // Sort waiting tasks by creation timestamp (FIFO)
        waiting_tasks.sort_by_key(|(_, created_at)| *created_at);

        let mut runnable = Vec::new();
        for (gid, _) in waiting_tasks {
            if max_concurrent > 0 && active_count >= max_concurrent {
                break; // Concurrency limit reached
            }
            runnable.push(gid);
            active_count += 1;
        }

        runnable
    }
}
