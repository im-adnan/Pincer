//! DownloadTask lifecycle & worker supervisor
//!
//! ### Architectural Overview
//! - **What it does**: Supervises multi-worker execution of a segmented download, orchestrating pre-allocation, bundle staging, worker spawning, and progress aggregation.
//! - **How it does**: Probes remote Content-Length, sets up staging `.download` bundles, partitions byte ranges, spawns `DownloadWorker`s, and returns progress channels and metadata to the caller.
//! - **Where it comes from**: Called by `manager::TaskRunner::run_task()` and `cli::DirectDownloader::run()`.
//! - **Where it leads to**: Returns active progress receivers and metadata, handing off execution to the caller's monitoring loop.

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::allocator::DiskAllocator;
use super::bundle::DownloadBundle;
use super::chunker::RangeChunker;
use super::worker::DownloadWorker;
use crate::common::{PincerError, PincerResult};
use crate::protocol::get_adapter;

/// Represents an executable download task descriptor and its configuration parameters.
pub struct DownloadTask {
    /// Mirror download URLs.
    pub urls: Vec<String>,
    /// Destination filename.
    pub filename: String,
    /// Destination directory path.
    pub save_path: String,
    /// Number of concurrent connection threads.
    pub threads: usize,
    /// Existing worker progress checkpoints.
    pub worker_progress: Vec<u64>,
    /// Custom HTTP request headers.
    pub headers: Vec<String>,
    /// Shared atomic bandwidth rate limit.
    pub global_limit: Arc<AtomicU64>,
    /// Shared atomic active worker thread counter.
    pub active_threads: Arc<AtomicU64>,
    /// Global engine configuration options.
    pub global_options: HashMap<String, String>,
}

impl DownloadTask {
    /// Initiates task execution and launches background download workers.
    ///
    /// Execution stages:
    /// 1. Obtains the appropriate protocol adapter for the primary URL.
    /// 2. Probes the remote endpoint for Content-Length, filename, and byte-range support.
    /// 3. If the server does not support byte ranges, falls back to a single worker (threads = 1).
    /// 4. Sets up macOS `.download` staging bundle structure and pre-allocates file length.
    /// 5. Calculates partitioned `RangeChunk` segments.
    /// 6. Spawns `DownloadWorker`s and returns a progress receiver channel `(worker_id, chunk_bytes)`.
    pub async fn start(
        &self,
        token: CancellationToken,
    ) -> PincerResult<(
        u64,
        usize,
        Option<String>,
        bool,
        mpsc::Receiver<(usize, u64)>,
        String,
    )> {
        let mut meta_opt = None;
        let mut working_url = String::new();
        let mut last_err = None;

        for url in &self.urls {
            match get_adapter(url) {
                Ok(adapter) => match adapter.get_content_length(url, &self.headers).await {
                    Ok(m) => {
                        meta_opt = Some(m);
                        working_url = url.clone();
                        break;
                    }
                    Err(e) => {
                        last_err = Some(e);
                    }
                },
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        let meta = meta_opt.ok_or_else(|| {
            last_err
                .unwrap_or_else(|| PincerError::InvalidUrl("All URLs failed metadata probe".into()))
        })?;
        let primary_url = &working_url;

        let total_size = meta.content_length.unwrap_or(0);
        let is_resumable = meta.resumable;

        // Force single thread if remote server does not support byte-range requests
        let actual_threads = if is_resumable && total_size > 0 {
            self.threads.max(1)
        } else {
            1
        };

        // Initialize .download staging bundle
        let (staged_file_path, relative_staged_name) =
            DownloadBundle::setup_bundle(&self.save_path, &self.filename, primary_url);

        // Pre-allocate contiguous disk space for the target file
        let file = DiskAllocator::allocate(&staged_file_path, total_size)?;
        let file_arc = Arc::new(file);

        // Calculate byte ranges for each worker
        let chunks = if is_resumable && total_size > 0 {
            RangeChunker::calculate_chunks(total_size, actual_threads, &self.worker_progress)
        } else {
            vec![super::chunker::RangeChunk {
                worker_id: 0,
                start: 0,
                end: if total_size > 0 { total_size - 1 } else { 0 },
            }]
        };

        let (progress_tx, progress_rx) = mpsc::channel(128);

        // Spawn parallel worker tasks
        for (i, chunk) in chunks.into_iter().enumerate() {
            let mut worker_urls = self.urls.clone();
            if !worker_urls.is_empty() {
                let len = worker_urls.len();
                worker_urls.rotate_left(i % len);
            }

            DownloadWorker::spawn(crate::engine::worker::WorkerConfig {
                urls: worker_urls,
                chunk,
                headers: self.headers.clone(),
                file: file_arc.clone(),
                progress_tx: progress_tx.clone(),
                token: token.clone(),
                global_limit: self.global_limit.clone(),
                active_threads: self.active_threads.clone(),
            });
        }

        Ok((
            total_size,
            actual_threads,
            meta.file_type,
            is_resumable,
            progress_rx,
            relative_staged_name,
        ))
    }
}
