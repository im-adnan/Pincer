use crate::protocol::ProtocolAdapter;
use futures::StreamExt;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// DownloadWorker represents a single connection thread downloading a specific byte range of a file.
/// It streams the HTTP response directly to disk using thread-safe offset writing (`write_at`).
pub struct DownloadWorker {
    pub id: usize,
    pub sources: Vec<(String, Arc<dyn ProtocolAdapter>)>,
    pub start: u64,
    pub end: u64,
    pub file: Arc<File>,
    pub progress_tx: mpsc::Sender<(usize, u64)>, // (worker_id, bytes_downloaded_this_tick)
    pub token: CancellationToken,
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
}

/// A helper RAII guard that safely increments the active thread count when created,
/// and automatically decrements it when dropped. Used for dynamic bandwidth distribution.
struct ThreadGuard {
    counter: Arc<AtomicU64>,
}

impl ThreadGuard {
    fn new(counter: Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for ThreadGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
}

impl DownloadWorker {
    /// Executes the worker thread. Connects to the server via the protocol adapter,
    /// requests its specific byte range, and streams chunks to the file offset while yielding to global speed limits.
    pub async fn run(self) -> Result<(), String> {
        let _guard = ThreadGuard::new(self.active_threads.clone());

        let mut current_offset = self.start;
        let mut last_err = String::new();

        for (url, adapter) in &self.sources {
            // Attempt to download chunk from this source
            let mut stream = match adapter.download_chunk(url, current_offset, self.end).await {
                Ok(s) => s,
                Err(e) => {
                    last_err = e;
                    continue; // Try next source
                }
            };

            #[allow(unused_assignments)]
            let mut source_failed = false;

            loop {
                let start_chunk = std::time::Instant::now();
                let chunk_res = stream.next().await;
                let download_duration = start_chunk.elapsed();

                let chunk = match chunk_res {
                    Some(Ok(c)) => c,
                    Some(Err(e)) => {
                        last_err = e;
                        source_failed = true;
                        break; // Break inner loop, try next source
                    }
                    None => {
                        return Ok(()); // Chunk fully downloaded
                    }
                };

                // Check for cancellation
                if self.token.is_cancelled() {
                    return Ok(());
                }

                let chunk_len = chunk.len() as u64;

                // Write chunk to disk safely
                if let Err(e) = self.file.write_at(&chunk, current_offset) {
                    return Err(format!("Failed to write to file: {}", e)); // Unrecoverable I/O error
                }

                current_offset += chunk_len;

                // Report progress back to the orchestrator
                let _ = self.progress_tx.send((self.id, chunk_len)).await;

                // --- Global Throttling Logic ---
                let global_limit = self.global_limit.load(Ordering::Relaxed);
                if global_limit > 0 {
                    let thread_count = self.active_threads.load(Ordering::Relaxed).max(1);
                    // Divide work budget among active threads
                    let per_thread_limit = global_limit / thread_count;

                    if let Some(target_ms) = (chunk_len * 1000).checked_div(per_thread_limit) {
                        let actual_ms = download_duration.as_millis() as u64;

                        if target_ms > actual_ms {
                            let sleep_ms = target_ms - actual_ms;
                            if sleep_ms > 0 {
                                tokio::select! {
                                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(sleep_ms)) => {},
                                    _ = self.token.cancelled() => return Ok(()),
                                }
                            }
                        }
                    } else {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }

            if !source_failed {
                return Ok(());
            }
        }

        Err(format!("All sources failed. Last error: {}", last_err))
    }
}
