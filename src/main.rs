mod models;
mod manager;
mod rpc;
mod worker;
mod task;

use manager::DownloadManager;
use std::env;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    
    // Command line mode for quick direct download tests
    if args.len() > 1 {
        let url = args[1].clone();
        println!("CLI Mode: Downloading {}...", url);
        
        let filename = url.split('/').last().unwrap_or("cli_download.bin").split('?').next().unwrap_or("cli_download.bin").to_string();
        let dir = std::env::current_dir().unwrap().to_str().unwrap().to_string();
        
        let task = crate::task::DownloadTask {
            url,
            filename,
            save_path: dir,
            threads: 4,
            resume_offset: 0,
            global_limit: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active_threads: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)), // 1 thread active in CLI
        };
        
        let token = tokio_util::sync::CancellationToken::new();
        match task.start(token).await {
            Ok((_, mut progress_rx)) => {
                while let Some((_worker_id, _bytes_chunk)) = progress_rx.recv().await {
                    // Quick CLI progress print
                    // print!("."); 
                }
                println!("\nDownload complete!");
            },
            Err(e) => {
                eprintln!("Download failed: {}", e);
            }
        }
        return;
    }

    // Default Server mode
    println!("Starting Pincer engine...");
    let (manager, rx) = DownloadManager::new();
    
    // Start WebSocket server to listen for Sluice RPC
    rpc::start_server(manager, rx).await;
}
