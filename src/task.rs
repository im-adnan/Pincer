use reqwest::Client;
use std::fs::OpenOptions;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::protocol::{FtpAdapter, HttpAdapter, ProtocolAdapter, SftpAdapter};
use crate::worker::DownloadWorker;

/// Represents the orchestrator for a single download instance.
/// It handles metadata discovery, pre-allocates disk space, calculates byte ranges,
/// and spawns one or more `DownloadWorker` asynchronous tasks.
pub struct DownloadTask {
    pub urls: Vec<String>,
    pub filename: String,
    pub save_path: String,
    pub threads: usize,
    pub worker_progress: Vec<u64>,
    pub headers: Vec<String>,
    pub global_limit: Arc<AtomicU64>,
    pub active_threads: Arc<AtomicU64>,
    pub global_options: std::collections::HashMap<String, String>,
}

impl DownloadTask {
    /// Initiates the download lifecycle.
    /// Discovers metadata (Content-Length, Accept-Ranges) using HTTP HEAD/GET, allocates the file on disk,
    /// divides the remaining byte range among the requested threads, and spawns the workers.
    /// Returns a channel receiver to stream progress updates back to the orchestrator.
    pub async fn start(
        self,
        token: CancellationToken,
    ) -> Result<
        (
            u64,
            usize,
            Option<String>,
            bool,
            mpsc::Receiver<(usize, u64)>,
            String, // part_filename (e.g. "file.mkv.pincer")
        ),
        String,
    > {
        use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
        use std::str::FromStr;

        let mut header_map = HeaderMap::new();
        for h in &self.headers {
            if let Some((k, v)) = h.split_once(':') {
                if let (Ok(name), Ok(value)) = (
                    HeaderName::from_str(k.trim()),
                    HeaderValue::from_str(v.trim()),
                ) {
                    header_map.insert(name, value);
                }
            }
        }

        let mut sources = Vec::new();
        for u in &self.urls {
            let adapter: Arc<dyn ProtocolAdapter> = if u.starts_with("ftp://") {
                Arc::new(FtpAdapter::new())
            } else if u.starts_with("sftp://") {
                Arc::new(SftpAdapter::new())
            } else {
                let mut client_builder = Client::builder();
                if let Some(ua) = self
                    .global_options
                    .get("user-agent")
                    .filter(|s| !s.is_empty())
                {
                    client_builder = client_builder.user_agent(ua);
                } else if !header_map.contains_key(reqwest::header::USER_AGENT) {
                    client_builder = client_builder.user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
                }
                if let Some(proxy_url) = self
                    .global_options
                    .get("all-proxy")
                    .filter(|s| !s.is_empty())
                {
                    if let Ok(proxy) = reqwest::Proxy::all(proxy_url) {
                        client_builder = client_builder.proxy(proxy);
                    }
                }
                let client = client_builder
                    .default_headers(header_map.clone())
                    .build()
                    .map_err(|e| e.to_string())?;
                Arc::new(HttpAdapter::new(client))
            };
            sources.push((u.clone(), adapter));
        }

        if sources.is_empty() {
            return Err("No URLs provided for task.".to_string());
        }

        // Phase A: Discovery
        let mut metadata = None;
        let mut last_err = String::new();
        for (u, adapter) in &sources {
            match adapter.resolve_metadata(u, &self.headers).await {
                Ok(m) => {
                    metadata = Some(m);
                    break;
                }
                Err(e) => {
                    last_err = e;
                }
            }
        }

        let metadata = metadata.ok_or_else(|| {
            format!(
                "Failed to resolve metadata from all sources. Last error: {}",
                last_err
            )
        })?;

        let content_length = metadata.total_size.unwrap_or(0);
        if content_length == 0 {
            return Err("Could not determine file size. Server must support Content-Length or Size commands.".to_string());
        }

        let supports_ranges = metadata.is_resumable.unwrap_or(false);

        // Determine thread count (fallback to 1 if ranges aren't supported)
        let actual_threads = if supports_ranges && content_length > 0 {
            self.threads
        } else {
            1
        };

        // Phase B: Determine filename and open for writing.
        // NATIVE MACOS .DOWNLOAD BUNDLE APPROACH: We create a .download directory wrapper
        // and place an Info.plist inside so macOS native Finder shows the progress pie
        // without crashing indexing daemons.
        let download_bundle_name = format!("{}.download", self.filename);
        let part_filename = format!("{}/.{}.part", download_bundle_name, self.filename);

        let bundle_path_str = format!("{}/{}", self.save_path, download_bundle_name);
        let bundle_path = Path::new(&bundle_path_str);
        let file_path_str = format!("{}/{}", self.save_path, part_filename);
        let path = Path::new(&file_path_str);

        // 1. Create the bundle directory if it doesn't exist
        if !bundle_path.exists() {
            let _ = std::fs::create_dir_all(bundle_path);
        }

        // 2. Create the Info.plist inside the bundle
        let plist_path = bundle_path.join("Info.plist");
        if !plist_path.exists() {
            let ext = Path::new(&self.filename)
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_lowercase();
            let uti = match ext.as_str() {
                "mp4" | "m4v" => "public.mpeg-4",
                "mkv" => "public.movie",
                "mp3" => "public.mp3",
                "zip" => "com.pkware.zip-archive",
                "pdf" => "com.adobe.pdf",
                "png" => "public.png",
                "jpg" | "jpeg" => "public.jpeg",
                "gif" => "com.compuserve.gif",
                "dmg" => "com.apple.disk-image",
                _ => "public.data",
            };

            let plist_content = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>DownloadTitle</key>
    <string>{}</string>
    <key>DownloadTargetUTI</key>
    <string>{}</string>
</dict>
</plist>"#,
                self.filename, uti
            );
            let _ = std::fs::write(&plist_path, plist_content);
        }

        // 3. Set com.apple.quarantine xattr on the BUNDLE directory so macOS/Finder marks this as downloading.
        #[cfg(target_os = "macos")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            // Official Apple format: flags;timestamp;agent_name;UUID
            let uuid = uuid::Uuid::new_v4().to_string().to_uppercase();
            let qtn = format!("0083;{:08x};PincerEngine;{}", ts, uuid);
            if let Err(e) = xattr::set(bundle_path, "com.apple.quarantine", qtn.as_bytes()) {
                eprintln!(
                    "Warning: Failed to set quarantine xattr on bundle dir: {}",
                    e
                );
            }
        }

        // 4. Open the actual hidden part file for writing the download chunks
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("Failed to open target file: {}", e))?;

        // Pre-allocate file space on the .pincer file if we are not resuming
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if current_len < content_length {
            file.set_len(content_length)
                .map_err(|e| format!("Failed to allocate file size: {}", e))?;
        }

        // Wrap file in an Arc to safely share across worker threads for Unix write_at (thread-safe)
        let shared_file = Arc::new(file);

        let (progress_tx, progress_rx) = mpsc::channel(100);

        // Phase C & D: Chunking and Spawning
        // Divide original file into equal chunks, and offset by individual worker progress
        let total_completed: u64 = self.worker_progress.iter().sum();
        if total_completed >= content_length && content_length > 0 {
            let file_type = metadata.file_type.clone();
            return Ok((
                content_length,
                actual_threads,
                file_type,
                supports_ranges,
                progress_rx,
                part_filename,
            )); // Already done
        }

        let chunk_size = content_length / actual_threads as u64;
        let mut handles: Vec<JoinHandle<Result<(), String>>> = vec![];

        for i in 0..actual_threads {
            let original_start = i as u64 * chunk_size;
            let original_end = if i == actual_threads - 1 {
                content_length.saturating_sub(1)
            } else {
                original_start + chunk_size - 1
            };

            let completed = if supports_ranges {
                self.worker_progress.get(i).cloned().unwrap_or(0)
            } else {
                0
            };

            let start = original_start + completed;
            let end = original_end;

            if start > end {
                continue; // This thread has finished its work
            }

            let worker = DownloadWorker {
                id: i,
                sources: sources.clone(),
                start,
                end,
                file: shared_file.clone(),
                progress_tx: progress_tx.clone(),
                token: token.clone(),
                global_limit: self.global_limit.clone(),
                active_threads: self.active_threads.clone(),
            };

            // Spawn the tokio task
            let handle = tokio::spawn(async move { worker.run().await });

            handles.push(handle);
        }

        // We could await handles here, or let them run detached.
        // Returning the progress receiver allows the caller (Manager) to track and hold the loop.
        let file_type = metadata.file_type;

        Ok((
            content_length,
            actual_threads,
            file_type,
            supports_ranges,
            progress_rx,
            part_filename,
        ))
    }
}
