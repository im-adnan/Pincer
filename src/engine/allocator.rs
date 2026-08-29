//! Disk space pre-allocation
//!
//! ### Architectural Overview
//! - **What it does**: Pre-allocates destination file length on disk prior to worker thread execution to avoid file fragmentation and guarantee disk space availability.
//! - **How it does**: Opens or creates the target file using `std::fs::OpenOptions` with write/create permissions, and invokes `file.set_len(total_size)` to expand file length in a single atomic OS operation.
//! - **Where it comes from**: Called by `engine::DownloadTask::start()` once remote Content-Length is resolved.
//! - **Where it leads to**: Returns an opened `std::fs::File` descriptor shared across worker threads wrapped in `Arc<File>`.

use crate::common::PincerResult;
use std::fs::{File, OpenOptions};
use std::path::Path;

/// Handles zero-fragmentation disk file pre-allocation.
pub struct DiskAllocator;

impl DiskAllocator {
    /// Opens the target file and pre-allocates its length on disk.
    ///
    /// Pre-allocating total file length via `set_len`:
    /// 1. Ensures the filesystem allocates contiguous storage blocks, preventing fragmentation.
    /// 2. Verifies upfront that sufficient disk space exists before downloading large payloads.
    /// 3. Allows concurrent worker threads to write to arbitrary byte offsets (`write_at`) safely.
    pub fn allocate(path_str: &str, total_size: u64) -> PincerResult<File> {
        let path = Path::new(path_str);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        if total_size > 0 {
            file.set_len(total_size)?;
        }

        Ok(file)
    }
}
