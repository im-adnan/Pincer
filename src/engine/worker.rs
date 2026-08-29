//! DownloadWorker execution loop & write_at
//!
//! ### Architectural Overview
//! - **What it does**: Executes an individual asynchronous download worker thread, streaming byte ranges from a `ProtocolAdapter` and committing data to disk.
//! - **How it does**: Performs zero-allocation concurrent writes using POSIX `write_at` (via `std::os::unix::fs::FileExt`) directly on an `Arc<File>` across threads without mutex lock contention, while obeying cancellation tokens and bandwidth throttles.
//! - **Where it comes from**: Spawned by `engine::DownloadTask::start()` for each computed `RangeChunk`.
//! - **Where it leads to**: Writes downloaded bytes directly into the target file descriptor and transmits `(worker_id, chunk_len)` progress tuples to the task channel.

use futures::StreamExt;
use std::fs::File;
#[cfg(unix)]
use std::os::unix::fs::FileExt;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::chunker::RangeChunk;
use super::throttler::{RateThrottler, ThreadGuard};
use crate::common::PincerResult;

/// Asynchronous worker task responsible for streaming and writing a single byte range segment.
pub struct DownloadWorker;

/// Configuration parameters for a single download worker thread.
pub struct WorkerConfig {
    pub urls: Vec<String>,
    pub chunk: RangeChunk,
    pub headers: Vec<String>,
    pub file: Arc<File>,
    pub progress_tx: mpsc::Sender<(usize, u64)>,
    pub token: CancellationToken,
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
}

impl DownloadWorker {
    /// Spawns a background worker task that streams and writes a byte chunk segment.
    ///
    /// Execution stages:
    /// 1. Instantiates RAII `ThreadGuard` to register this worker in active thread metrics.
    /// 2. Requests byte stream from the protocol adapter for `[chunk.start, chunk.end]`.
    /// 3. In a loop, receives byte chunks from the network stream:
    ///    - Checks for task cancellation.
    ///    - Applies bandwidth rate limiting via `RateThrottler::throttle()`.
    ///    - Writes data directly to file offset via POSIX `write_at` in `spawn_blocking`.
    ///    - Increments offset and transmits `(worker_id, bytes_len)` progress tuple.
    /// 4. Flushes file data buffers to disk upon completion.
    pub fn spawn(config: WorkerConfig) -> tokio::task::JoinHandle<PincerResult<()>> {
        let WorkerConfig {
            urls,
            chunk,
            headers,
            file,
            progress_tx,
            token,
            global_limit,
            active_threads,
        } = config;

        tokio::spawn(async move {
            let _guard = ThreadGuard::new(active_threads.clone());
            let mut current_offset = chunk.start;
            let mut last_err = None;
            let max_attempts = std::cmp::max(urls.len() * 2, 1);
            let mut is_completed = false;

            for url in urls.into_iter().cycle().take(max_attempts) {
                if current_offset >= chunk.end {
                    is_completed = true;
                    break;
                }

                let adapter = match crate::protocol::get_adapter(&url) {
                    Ok(a) => a,
                    Err(e) => {
                        last_err = Some(e);
                        continue;
                    }
                };

                let mut stream = match adapter
                    .get_stream(&url, current_offset, chunk.end, &headers)
                    .await
                {
                    Ok(s) => s,
                    Err(e) => {
                        last_err = Some(e);
                        continue;
                    }
                };

                let mut last_chunk_time = Instant::now();
                let mut stream_failed = false;

                while let Some(chunk_res) = stream.next().await {
                    if token.is_cancelled() {
                        return Ok(());
                    }

                    let data = match chunk_res {
                        Ok(d) => d,
                        Err(e) => {
                            last_err = Some(e);
                            stream_failed = true;
                            break;
                        }
                    };

                    let len = data.len();
                    RateThrottler::throttle(
                        len,
                        &mut last_chunk_time,
                        &global_limit,
                        &active_threads,
                    )
                    .await;

                    let file_ref = file.clone();
                    let write_offset = current_offset;
                    let data_clone = data.clone();

                    let write_res = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                        #[cfg(unix)]
                        file_ref.write_all_at(&data_clone, write_offset)?;
                        Ok(())
                    })
                    .await
                    .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));

                    if let Err(e) = write_res {
                        last_err = Some(crate::common::PincerError::Other(e.to_string()));
                        stream_failed = true;
                        break;
                    }

                    current_offset += len as u64;

                    if progress_tx
                        .send((chunk.worker_id, len as u64))
                        .await
                        .is_err()
                    {
                        return Ok(());
                    }
                }

                if !stream_failed {
                    is_completed = true;
                    break;
                }
            }

            if !is_completed && current_offset < chunk.end {
                return Err(last_err.unwrap_or_else(|| {
                    crate::common::PincerError::Other("All fallback mirrors failed".into())
                }));
            }

            let file_flush = file.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = file_flush.sync_data();
            })
            .await;

            Ok(())
        })
    }
}
