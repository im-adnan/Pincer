mod models;
mod manager;
mod rpc;
// mod worker; 
// mod task; 

use manager::DownloadManager;

#[tokio::main]
async fn main() {
    println!("Starting Pincer engine...");
    let (manager, rx) = DownloadManager::new();
    
    // Start WebSocket server to listen for Downly RPC
    rpc::start_server(manager, rx).await;
}
