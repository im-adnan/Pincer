//! TaskControl & internal task registry state
//!
//! ### Architectural Overview
//! - **What it does**: Holds in-memory task control state, cancellation tokens, options maps, instantaneous speed timestamps, and integrity checksums for each registered task.
//! - **How it does**: Packages `TaskStatus`, `CancellationToken`, custom options `HashMap`, last update timestamps, and creation times into a cohesive `TaskControl` struct.
//! - **Where it comes from**: Constructed by `manager::TaskSpawner` and stored in `manager.tasks` registry.
//! - **Where it leads to**: Accessed and modified by all manager operations (scheduler, query, lifecycle, speed calculator, session persistence).

use crate::models::TaskStatus;
use std::collections::HashMap;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Internal in-memory control structure for managing a single download task's state.
pub struct TaskControl {
    /// Publicly exposed task status descriptor.
    pub status: TaskStatus,
    /// Cooperative cancellation token used to pause or abort active workers.
    pub token: CancellationToken,
    /// Task-specific configuration options (e.g. "dir", "split", "header").
    pub options: HashMap<String, String>,
    /// Downloaded byte count at the time of the previous speed calculation interval.
    pub last_update_bytes: u64,
    /// Instant timestamp when byte progress was last recorded.
    pub last_update_time: Instant,
    /// Optional expected SHA256 integrity checksum.
    pub expected_hash: Option<String>,
    /// Epoch timestamp (milliseconds) when the task was initially created.
    pub created_at: u128,
}

impl TaskControl {
    /// Creates a new `TaskControl` instance with default speed calculation timestamps.
    pub fn new(
        status: TaskStatus,
        token: CancellationToken,
        options: HashMap<String, String>,
        expected_hash: Option<String>,
        created_at: u128,
    ) -> Self {
        Self {
            status,
            token,
            options,
            last_update_bytes: 0,
            last_update_time: Instant::now(),
            expected_hash,
            created_at,
        }
    }
}
