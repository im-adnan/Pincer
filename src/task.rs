use reqwest::Client;
use std::sync::atomic::AtomicU64;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::worker::DownloadWorker;

pub struct DownloadTask {
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub threads: usize,
    pub resume_offset: u64,
    pub headers: Vec<String>,
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
}

impl DownloadTask {
    pub async fn start(self, token: CancellationToken) -> Result<(u64, mpsc::Receiver<(usize, u64)>), String> {
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
        use std::str::FromStr;

        let mut header_map = HeaderMap::new();
        for h in &self.headers {
            if let Some((k, v)) = h.split_once(':') {
                if let (Ok(name), Ok(value)) = (HeaderName::from_str(k.trim()), HeaderValue::from_str(v.trim())) {
                    header_map.insert(name, value);
                }
            }
        }

        let mut client_builder = Client::builder();
        
        // Add a default User-Agent to avoid being blocked by CDNs
        if !header_map.contains_key(reqwest::header::USER_AGENT) {
            client_builder = client_builder.user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        }

        let client = client_builder
            .default_headers(header_map)
            .build()
            .map_err(|e| e.to_string())?;

        // Phase A: Discovery
        // Many video servers block HEAD but allow GET. We'll try HEAD first.
        let mut res = match client.head(&self.url).send().await {
            Ok(r) if r.status().is_success() => r,
            _ => {
                // Fallback to GET with a tiny range to discover metadata (Content-Length/Range)
                client.get(&self.url)
                    .header(reqwest::header::RANGE, "bytes=0-0")
                    .send()
                    .await
                    .map_err(|e| format!("Both HEAD and GET discovery failed: {}", e))?
            }
        };
        
        // If HEAD succeeded but doesn't have content length, try GET discovery
        if res.headers().get(reqwest::header::CONTENT_LENGTH).is_none() {
             res = client.get(&self.url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
                .map_err(|e| format!("GET discovery failed after empty HEAD: {}", e))?;
        }

        let content_length = if let Some(full_range) = res.headers().get("Content-Range") {
             // If we got a 206 from our bytes=0-0 fallback, the full size is after the '/' 
             // e.g. "bytes 0-0/12345"
             full_range.to_str().ok()
                .and_then(|s| s.split('/').last())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0)
        } else {
            res.headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|ct_len| ct_len.to_str().ok())
                .and_then(|ct_len| ct_len.parse::<u64>().ok())
                .unwrap_or(0)
        };

        if content_length == 0 {
            return Err("Could not determine file size. Server must support Content-Length or Content-Range.".to_string());
        }

        let supports_ranges = res.headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .map(|val| val == "bytes")
            .or_else(|| {
                // If we got a 206 Partial Content, ranges are supported
                if res.status() == 206 { Some(true) } else { None }
            })
            .unwrap_or(false);

        // Determine thread count (fallback to 1 if ranges aren't supported)
        let actual_threads = if supports_ranges && content_length > 0 { self.threads } else { 1 };
        
        let file_path_str = format!("{}/{}", self.save_path, self.filename);
        let path = Path::new(&file_path_str);

        // Phase B: Allocation
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .map_err(|e| format!("Failed to open file: {}", e))?;
        
        // Only set length if we are not resuming or if file is smaller than expected
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if current_len < content_length {
            file.set_len(content_length).map_err(|e| format!("Failed to allocate file size: {}", e))?;
        }
        
        // Wrap file in an Arc to safely share across worker threads for Unix write_at (thread-safe)
        let shared_file = Arc::new(file);

        let (progress_tx, progress_rx) = mpsc::channel(100);
        
        // Phase C & D: Chunking and Spawning
        // When resuming, we still want to use multi-threading for the REMAINING part.
        // Simplified approach: Divide the REMAINING bytes among threads.
        let remaining_size = if self.resume_offset < content_length {
            content_length - self.resume_offset
        } else {
            0
        };

        if remaining_size == 0 && self.resume_offset > 0 {
             return Ok((content_length, progress_rx)); // Already done
        }

        let chunk_size = remaining_size / actual_threads as u64;
        let mut handles: Vec<JoinHandle<Result<(), String>>> = vec![];

        for i in 0..actual_threads {
            let start = self.resume_offset + (i as u64 * chunk_size);
            let end = if i == actual_threads - 1 {
                content_length - 1
            } else {
                start + chunk_size - 1
            };

            let worker = DownloadWorker {
                id: i,
                url: self.url.clone(),
                start,
                end,
                file: shared_file.clone(),
                progress_tx: progress_tx.clone(),
                token: token.clone(),
                global_limit: self.global_limit.clone(),
                active_threads: self.active_threads.clone(),
            };
            
            let client_clone = client.clone();
            
            // Spawn the tokio task
            let handle = tokio::spawn(async move {
                worker.run(client_clone).await
            });
            
            handles.push(handle);
        }

        // We could await handles here, or let them run detached. 
        // Returning the progress receiver allows the caller (Manager) to track and hold the loop.
        Ok((content_length, progress_rx))
    }
}
