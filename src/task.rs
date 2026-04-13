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
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
}

impl DownloadTask {
    pub async fn start(self, token: CancellationToken) -> Result<(u64, mpsc::Receiver<(usize, u64)>), String> {
        let client = Client::new();

        // Phase A: Discovery
        let res = client.head(&self.url).send().await.map_err(|e| e.to_string())?;
        
        let content_length = res.headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|ct_len| ct_len.to_str().ok())
            .and_then(|ct_len| ct_len.parse::<u64>().ok())
            .unwrap_or(0);

        if content_length == 0 {
            return Err("Could not determine file size. Server must support Content-Length.".to_string());
        }

        let supports_ranges = res.headers()
            .get(reqwest::header::ACCEPT_RANGES)
            .map(|val| val == "bytes")
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
