mod manager;
mod models;
mod rpc;
mod task;
mod worker;

use lexopt::ValueExt;
use manager::DownloadManager;
use std::env;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_TYPE: &str = "dev";

/// Entry point for the Pincer application.
/// Parses CLI arguments using `lexopt`.
/// If a URL is provided, it executes in Direct CLI Mode (downloading immediately).
/// Otherwise, it starts the background WebSocket JSON-RPC server daemon.
#[tokio::main]
async fn main() -> Result<(), lexopt::Error> {
    let mut url = None;
    let mut split = 1;
    let mut dir = std::env::current_dir()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut out = None;
    let mut format = None;
    let mut log = false;

    let mut parser = lexopt::Parser::from_env();
    while let Some(arg) = parser.next()? {
        match arg {
            lexopt::Arg::Short('s') | lexopt::Arg::Long("split") => {
                split = parser.value()?.parse::<usize>()?.min(99);
            }
            lexopt::Arg::Short('d') | lexopt::Arg::Long("dir") => {
                dir = parser.value()?.string()?;
            }
            lexopt::Arg::Short('o') | lexopt::Arg::Long("out") => {
                out = Some(parser.value()?.string()?);
            }
            lexopt::Arg::Short('f') | lexopt::Arg::Long("format") => {
                format = Some(parser.value()?.string()?);
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
                println!(
                    "  -s, --split <N>     Number of concurrent threads (Default: 1, Max: 99)"
                );
                println!("  -d, --dir <DIR>     Target directory (Default: current)");
                println!("  -o, --out <FILE>    Custom output filename");
                println!("  -f, --format <FMT>  Target format to convert the downloaded file to");
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
        let (manager, _) = DownloadManager::new();

        if log {
            println!("\n  \x1b[1;34m🚀 PINCER\x1b[0m | \x1b[37mHigh-Performance Engine\x1b[0m");
            println!("  \x1b[90m───────────────────────────────────────────\x1b[0m");
            println!("  \x1b[34m[INFO]\x1b[0m Resolving metadata...");
        }

        // Phase 1: Resolve Metadata
        let resolved = match manager.resolve_url(url.clone()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  \x1b[31m[ERROR]\x1b[0m Failed to resolve URL: {}", e);
                std::process::exit(1);
            }
        };

        let final_url = resolved.url;
        let filename = out.unwrap_or_else(|| {
            resolved
                .filename
                .unwrap_or_else(|| "download.bin".to_string())
        });
        let total_size = resolved.total_size.unwrap_or(0) as u64;
        let file_type = resolved.file_type.unwrap_or_else(|| "unknown".to_string());
        let resumable = resolved.is_resumable.unwrap_or(false);

        if log {
            println!("  \x1b[32m[SUCCESS]\x1b[0m Metadata resolved!");
            println!("  \x1b[90m───────────────────────────────────────────\x1b[0m");
            println!("  \x1b[1;33m📦 File:      \x1b[0;37m{}", filename);
            println!(
                "  \x1b[1;33m📏 Size:      \x1b[0;37m{} \x1b[90m({})\x1b[0m",
                format_bytes(total_size),
                file_type
            );
            println!("  \x1b[1;33m⚡ Threads:   \x1b[0;37m{} Workers", split);
            println!("  \x1b[1;33m📂 Target:    \x1b[0;37m{}", dir);
            println!(
                "  \x1b[1;33m⏯️  Resumable: \x1b[0;37m{}",
                if resumable {
                    "\x1b[32mYes\x1b[0m"
                } else {
                    "\x1b[31mNo\x1b[0m"
                }
            );
            println!("  \x1b[90m───────────────────────────────────────────\x1b[0m\n");
            println!("  \x1b[34m[INFO]\x1b[0m Starting download...");
        }

        let task = crate::task::DownloadTask {
            url: final_url.clone(),
            filename: filename.clone(),
            save_path: dir.clone(),
            threads: split,
            resume_offset: 0,
            headers: vec![],
            global_limit: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active_threads: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(1)),
        };

        let token = tokio_util::sync::CancellationToken::new();
        match task.start(token.clone()).await {
            Ok((content_length, actual_threads, _file_type, _is_resumable, mut progress_rx)) => {
                let mut completed = 0;
                let mut thread_progress = vec![0u64; actual_threads];
                let start_time = std::time::Instant::now();
                let mut last_update = std::time::Instant::now();
                let mut bytes_since_last = 0;
                let mut speed = 0.0;

                let mut last_render = std::time::Instant::now();

                while let Some((worker_id, bytes_chunk)) = progress_rx.recv().await {
                    completed += bytes_chunk;
                    if worker_id < actual_threads {
                        thread_progress[worker_id] += bytes_chunk;
                    }
                    bytes_since_last += bytes_chunk;

                    let now = std::time::Instant::now();
                    let elapsed_last = now.duration_since(last_update).as_secs_f64();

                    if elapsed_last >= 0.5 {
                        speed = bytes_since_last as f64 / elapsed_last;
                        bytes_since_last = 0;
                        last_update = now;
                    }

                    // Performance Optimization: Only render the UI at most every 100ms
                    if log
                        && (now.duration_since(last_render).as_millis() >= 100
                            || completed == content_length)
                    {
                        last_render = now;
                        let percent = (completed as f64 / content_length as f64) * 100.0;
                        let bar_width = 30;
                        let filled = (percent / 100.0 * bar_width as f64) as usize;
                        let bar = format!(
                            "\x1b[32m{:█<filled$}\x1b[90m{:░<empty$}\x1b[0m",
                            "",
                            "",
                            filled = filled,
                            empty = bar_width - filled
                        );

                        let eta = if speed > 0.0 {
                            let remaining = content_length - completed;
                            format_duration((remaining as f64 / speed) as u64)
                        } else {
                            "--:--".to_string()
                        };

                        // Build thread status string
                        let mut thread_status = String::new();
                        for (i, p) in thread_progress.iter().enumerate() {
                            thread_status.push_str(&format!(
                                "\x1b[90m[\x1b[33mT{}:\x1b[37m{}\x1b[90m]\x1b[0m ",
                                i + 1,
                                format_bytes_compact(*p)
                            ));
                        }

                        print!("\r  {} \x1b[1;32m{:>5.1}%\x1b[0m\n  \x1b[36m⚡ {:>10}/s\x1b[0m | \x1b[35m⏳ ETA: {:<8}\x1b[0m | \x1b[37m{}/{}\x1b[0m\n\x1b[2K  \x1b[34m🧵 Workers:\x1b[0m {}\x1b[2A", 
                            bar, percent, format_bytes(speed as u64), eta, format_bytes(completed), format_bytes(content_length), thread_status);

                        use std::io::Write;
                        std::io::stdout().flush().unwrap();
                    }
                }

                let total_elapsed = start_time.elapsed();
                println!("\n\n\n  \x1b[1;32m✔ Download Complete!\x1b[0m \x1b[90m(Total Time: {})\x1b[0m\n", format_duration(total_elapsed.as_secs()));

                // Perform format conversion if requested in CLI options
                if let Some(target_fmt) = format {
                    let file_path = format!("{}/{}", dir, filename);
                    let target_filename = if let Some(dot_idx) = filename.rfind('.') {
                        format!("{}.{}", &filename[..dot_idx], target_fmt)
                    } else {
                        format!("{}.{}", filename, target_fmt)
                    };
                    let target_file_path = format!("{}/{}", dir, target_filename);

                    if file_path != target_file_path {
                        println!(
                            "  \x1b[34m[INFO]\x1b[0m Preparing conversion to format: {}...",
                            target_fmt
                        );
                        if let Err(e) = std::fs::rename(&file_path, &target_file_path) {
                            eprintln!(
                                "  \x1b[31m[ERROR]\x1b[0m Failed to prepare conversion: {}",
                                e
                            );
                        } else {
                            println!("  \x1b[34m[INFO]\x1b[0m Transcoding file now...");
                            manager
                                .perform_format_conversion(
                                    &target_file_path,
                                    &final_url,
                                    Some(&file_type),
                                )
                                .await;
                            println!(
                                "  \x1b[32m[SUCCESS]\x1b[0m Conversion completed successfully!"
                            );
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("\n  \x1b[31m✖ Download Failed: {}\x1b[0m", e);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Default Server mode
    println!(
        "\x1b[1;34mStarting Pincer engine v{} ({})\x1b[0m",
        VERSION, BUILD_TYPE
    );
    let (manager, rx) = DownloadManager::new();

    // Auto-load previously saved sessions
    manager.load_session().await;

    // Start WebSocket server to listen for RPC
    rpc::start_server(manager, rx).await;
    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.2} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_bytes_compact(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.0}K", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1}M", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}G", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}
