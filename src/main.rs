mod models;
mod manager;
mod rpc;
mod worker;
mod task;

use manager::DownloadManager;
use std::env;
use lexopt::ValueExt;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TYPE: &str = "dev";

#[tokio::main]
async fn main() -> Result<(), lexopt::Error> {
    let mut url = None;
    let mut split = 4;
    let mut dir = std::env::current_dir().unwrap().to_str().unwrap().to_string();
    let mut out = None;
    let mut log = false;

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            lexopt::Arg::Short('s') | lexopt::Arg::Long("split") => {
                split = parser.value()?.parse()?;
            }
            lexopt::Arg::Short('d') | lexopt::Arg::Long("dir") => {
                dir = parser.value()?.string()?;
            }
            lexopt::Arg::Short('o') | lexopt::Arg::Long("out") => {
                out = Some(parser.value()?.string()?);
            }
            lexopt::Arg::Short('l') | lexopt::Arg::Long("log") => {
                log = true;
            }
            lexopt::Arg::Short('v') | lexopt::Arg::Long("version") => {
                println!("pincer {} ({})", VERSION, BUILD_TYPE);
                return Ok(());
            }
            lexopt::Arg::Short('h') | lexopt::Arg::Long("help") => {
                println!("Pincer - High-performance download engine");
                println!("\nUsage: pincer [URL] [OPTIONS]");
                println!("\nOptions:");
                println!("  -s, --split <N>     Number of concurrent threads (Default: 4)");
                println!("  -d, --dir <DIR>     Target directory (Default: current)");
                println!("  -o, --out <FILE>    Custom output filename");
                println!("  -l, --log           Enable detailed logging");
                println!("  -v, --version       Print version information");
                println!("  -h, --help          Print help information");
                return Ok(());
            }
            lexopt::Arg::Value(val) if url.is_none() => {
                url = Some(val.string()?);
            }
            _ => return Err(arg.unexpected()),
        }
    }

    // CLI Mode (Direct Download)
    if let Some(url) = url {
        if log {
            println!("🚀 Pincer CLI Mode");
            println!("URL: {}", url);
            println!("Threads: {}", split);
            println!("Directory: {}", dir);
            if let Some(ref o) = out { println!("Output: {}", o); }
            println!("-------------------------------------------");
        }

        let filename = out.unwrap_or_else(|| {
            url.split('/').last().unwrap_or("download.bin").split('?').next().unwrap_or("download.bin").to_string()
        });
        
        let task = crate::task::DownloadTask {
            url,
            filename,
            save_path: dir,
            threads: split,
            resume_offset: 0,
            headers: vec![],
            global_limit: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active_threads: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };
        
        let token = tokio_util::sync::CancellationToken::new();
        match task.start(token).await {
            Ok((_, mut progress_rx)) => {
                while let Some((_worker_id, _bytes_chunk)) = progress_rx.recv().await {
                    // Progress
                }
                println!("\n✅ Download complete!");
            },
            Err(e) => {
                eprintln!("❌ Download failed: {}", e);
            }
        }
        return Ok(());
    }

    // Default Server mode
    println!("Starting Pincer engine v{} ({})", VERSION, BUILD_TYPE);
    let (manager, rx) = DownloadManager::new();
    
    // Start WebSocket server to listen for RPC
    rpc::start_server(manager, rx).await;
    Ok(())
}
