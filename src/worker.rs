use reqwest::header::{RANGE, HeaderValue};
use reqwest::Client;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct DownloadWorker {
    pub id: usize,
    pub url: String,
    pub start: u64,
    pub end: u64,
    pub file: Arc<std::sync::Mutex<File>>,
    pub progress_tx: mpsc::Sender<(usize, u64)>, // (worker_id, bytes_downloaded_this_tick)
    pub token: CancellationToken,
}

impl DownloadWorker {
    pub async fn run(self, client: Client) -> Result<(), String> {
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

        while let Some(chunk) = res.chunk().await.map_err(|e| e.to_string())? {
            // Check for cancellation
            if self.token.is_cancelled() {
                return Ok(());
            }

            let chunk_len = chunk.len() as u64;
            
            // Critical Section: Write chunk to disk safely
            {
                let file = self.file.lock().map_err(|_| "Failed to lock file".to_string())?;
                file.write_at(&chunk, current_offset)
                    .map_err(|e| format!("Failed to write to file: {}", e))?;
            }

            current_offset += chunk_len;
            
            // Report progress back to the orchestrator
            let _ = self.progress_tx.send((self.id, chunk_len)).await;
        }

        Ok(())
    }
}
