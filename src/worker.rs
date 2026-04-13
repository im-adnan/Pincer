use reqwest::header::{RANGE, HeaderValue};
use reqwest::Client;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct DownloadWorker {
    pub id: usize,
    pub url: String,
    pub start: u64,
    pub end: u64,
    pub file: Arc<File>,
    pub progress_tx: mpsc::Sender<(usize, u64)>, // (worker_id, bytes_downloaded_this_tick)
    pub token: CancellationToken,
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
}

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
    pub async fn run(self, client: Client) -> Result<(), String> {
        let _guard = ThreadGuard::new(self.active_threads.clone());
        
        // Construct the Range header for this segment
        let range_val = format!("bytes={}-{}", self.start, self.end);
        let mut req = client.get(&self.url);
        
        if let Ok(hv) = HeaderValue::from_str(&range_val) {
            req = req.header(RANGE, hv);
        }

            let mut res = req.send().await.map_err(|e| e.to_string())?;

            // If we requested a range, we expect 206 Partial Content.
            // If the server returns 200 OK, it means it doesn't support ranges and is sending the whole file.
            if self.start > 0 && res.status() == reqwest::StatusCode::OK {
                return Err("Server does not support resuming (returned 200 OK instead of 206 Partial Content)".to_string());
            }

            if !res.status().is_success() {
                return Err(format!("Server returned error: {}", res.status()));
            }

            let mut current_offset = self.start;

            loop {
                let start_chunk = std::time::Instant::now();
                let chunk_res = res.chunk().await.map_err(|e| e.to_string())?;
                let download_duration = start_chunk.elapsed();
                
                let chunk = match chunk_res {
                    Some(c) => c,
                    None => break,
                };

                // Check for cancellation
                if self.token.is_cancelled() {
                    return Ok(());
                }

                let chunk_len = chunk.len() as u64;
                
                // Write chunk to disk safely (write_at is thread-safe on Unix/macOS)
                self.file.write_at(&chunk, current_offset)
                    .map_err(|e| format!("Failed to write to file: {}", e))?;

                current_offset += chunk_len;
                
                // Report progress back to the orchestrator
                let _ = self.progress_tx.send((self.id, chunk_len)).await;

                // --- Global Throttling Logic ---
                let global_limit = self.global_limit.load(Ordering::Relaxed);
                if global_limit > 0 {
                    let thread_count = self.active_threads.load(Ordering::Relaxed).max(1);
                    // Divide work budget among active threads
                    let per_thread_limit = global_limit / thread_count;
                    
                    if per_thread_limit > 0 {
                        // target_duration (ms) = (bytes * 1000) / per_thread_limit
                        let target_ms = (chunk_len * 1000) / per_thread_limit;
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
                        // If limit is extremely low (e.g. Min mode) and we have many threads, 
                        // per_thread_limit might be 0. In this case, sleep a hard 100ms to avoid spinning.
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }

        Ok(())
    }
}
