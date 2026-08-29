//! Direct CLI download orchestrator
//!
//! ### Architectural Overview
//! - **What it does**: Executes direct, synchronous CLI downloads with live terminal progress reporting, automatic quarantine clearance, and optional post-download format transcoding.
//! - **How it does**: Resolves URL metadata via `manager.resolve_url()`, spawns a `DownloadTask`, polls worker progress channels to compute real-time speed and drive `TerminalProgressBar`, clears macOS quarantine xattrs, and delegates to `manager.perform_format_conversion()`.
//! - **Where it comes from**: Called by `cli::CliDispatcher::parse_and_dispatch()` when a URL positional argument is passed on the command line.
//! - **Where it leads to**: Outputs downloaded artifacts directly to disk, cleans up `.download` staging bundles, and prints terminal completion summaries.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

use super::args::CliArgs;
use super::formatters::{format_bytes, format_duration};
use super::progress_bar::TerminalProgressBar;
use crate::common::sanitize_filename;
use crate::engine::DownloadTask;
use crate::manager::DownloadManager;

/// Handles standalone CLI download operations with interactive terminal progress and format conversion.
pub struct DirectDownloader;

impl DirectDownloader {
    /// Executes a direct download session from start to finish.
    ///
    /// Pipeline stages:
    /// 1. Resolve remote metadata (detect redirects, filename, content length, resumability).
    /// 2. Display summary banner if logging is enabled.
    /// 3. Construct and launch `DownloadTask` with worker threads.
    /// 4. Poll chunk progress events to update UI progress bars and calculate instant speed.
    /// 5. Promote staged download from `.download` directory to final destination.
    /// 6. Strip macOS Gatekeeper quarantine attribute.
    /// 7. Perform optional post-download format transcoding if `--format` is specified.
    pub async fn run(url: String, args: CliArgs, manager: Arc<DownloadManager>) {
        if args.log {
            println!("\n  \x1b[1;34m🚀 PINCER\x1b[0m | \x1b[37mHigh-Performance Engine\x1b[0m");
            println!("  \x1b[90m───────────────────────────────────────────\x1b[0m");
            println!("  \x1b[34m[INFO]\x1b[0m Resolving metadata...");
        }

        // Step 1: Resolve metadata (size, filename, resumability, file type)
        let resolved = match manager.resolve_url(url.clone()).await {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  \x1b[31m[ERROR]\x1b[0m Failed to resolve URL: {}", e);
                std::process::exit(1);
            }
        };

        let final_url = resolved.url;
        let raw_filename = args.out.unwrap_or_else(|| {
            resolved
                .filename
                .unwrap_or_else(|| "download.bin".to_string())
        });
        let filename = sanitize_filename(&raw_filename);
        let total_size = resolved.total_size.unwrap_or(0) as u64;
        let file_type = resolved.file_type.unwrap_or_else(|| "unknown".to_string());
        let resumable = resolved.is_resumable.unwrap_or(false);

        // Step 2: Print download metadata summary
        if args.log {
            println!("  \x1b[32m[SUCCESS]\x1b[0m Metadata resolved!");
            println!("  \x1b[90m───────────────────────────────────────────\x1b[0m");
            println!("  \x1b[1;33m📦 File:      \x1b[0;37m{}", filename);
            println!(
                "  \x1b[1;33m📏 Size:      \x1b[0;37m{} \x1b[90m({})\x1b[0m",
                format_bytes(total_size),
                file_type
            );
            println!("  \x1b[1;33m⚡ Threads:   \x1b[0;37m{} Workers", args.split);
            println!("  \x1b[1;33m📂 Target:    \x1b[0;37m{}", args.dir);
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

        // Step 3: Instantiate DownloadTask descriptor
        let task = DownloadTask {
            urls: vec![final_url.clone()],
            filename: filename.clone(),
            save_path: args.dir.clone(),
            threads: args.split,
            worker_progress: vec![0; args.split],
            headers: vec![],
            global_limit: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active_threads: Arc::new(std::sync::atomic::AtomicU64::new(1)),
            global_options: std::collections::HashMap::new(),
        };

        let token = CancellationToken::new();
        match task.start(token).await {
            Ok((
                content_length,
                actual_threads,
                _file_type,
                _is_resumable,
                mut progress_rx,
                part_filename,
            )) => {
                let mut completed = 0;
                let mut thread_progress = vec![0u64; actual_threads];
                let start_time = Instant::now();
                let mut last_update = Instant::now();
                let mut bytes_since_last = 0;
                let mut speed = 0.0;
                let mut last_render = Instant::now();

                // Step 4: Stream worker progress chunks and calculate speeds
                while let Some((worker_id, bytes_chunk)) = progress_rx.recv().await {
                    completed += bytes_chunk;
                    if worker_id < actual_threads {
                        thread_progress[worker_id] += bytes_chunk;
                    }
                    bytes_since_last += bytes_chunk;

                    let now = Instant::now();
                    let elapsed_last = now.duration_since(last_update).as_secs_f64();

                    // Smooth speed calculation over 500ms intervals
                    if elapsed_last >= 0.5 {
                        speed = bytes_since_last as f64 / elapsed_last;
                        bytes_since_last = 0;
                        last_update = now;
                    }

                    // Render terminal progress bar at max 10Hz (every 100ms) or on completion
                    if args.log
                        && (now.duration_since(last_render).as_millis() >= 100
                            || completed == content_length)
                    {
                        last_render = now;
                        TerminalProgressBar::render(
                            completed,
                            content_length,
                            speed,
                            &thread_progress,
                        );
                    }
                }

                let total_elapsed = start_time.elapsed();
                println!(
                    "\n\n\n  \x1b[1;32m✔ Download Complete!\x1b[0m \x1b[90m(Total Time: {})\x1b[0m\n",
                    format_duration(total_elapsed.as_secs())
                );

                // Step 5: Finalize staging and rename .part to final filename
                let final_path = format!("{}/{}", args.dir, filename);
                let file_path = format!("{}/{}", args.dir, part_filename);

                if Path::new(&file_path).exists() && file_path != final_path {
                    let _ = std::fs::rename(&file_path, &final_path);
                }

                crate::engine::DownloadBundle::cleanup_bundle(&args.dir, &filename);

                // Step 6: Remove macOS quarantine xattr so downloaded files open smoothly
                #[cfg(target_os = "macos")]
                if Path::new(&final_path).exists() {
                    let _ = xattr::remove(&final_path, "com.apple.quarantine");
                }

                // Step 7: Perform format transcoding if requested
                if let Some(target_fmt) = args.format {
                    let target_filename = if let Some(dot_idx) = filename.rfind('.') {
                        format!("{}.{}", &filename[..dot_idx], target_fmt)
                    } else {
                        format!("{}.{}", filename, target_fmt)
                    };
                    let target_file_path = format!("{}/{}", args.dir, target_filename);

                    if final_path != target_file_path {
                        println!(
                            "  \x1b[34m[INFO]\x1b[0m Preparing conversion to format: {}...",
                            target_fmt
                        );
                        if let Err(e) = std::fs::rename(&final_path, &target_file_path) {
                            eprintln!(
                                "  \x1b[31m[ERROR]\x1b[0m Failed to prepare conversion: {}",
                                e
                            );
                        } else {
                            println!("  \x1b[34m[INFO]\x1b[0m Transcoding file now...");
                            if let Err(e) = manager
                                .perform_format_conversion(
                                    &target_file_path,
                                    &final_url,
                                    Some(&file_type),
                                )
                                .await
                            {
                                eprintln!("  \x1b[31m[ERROR]\x1b[0m {}", e);
                            } else {
                                println!(
                                    "  \x1b[32m[SUCCESS]\x1b[0m Conversion completed successfully!"
                                );
                            }
                        }
                    }
                }
            }
            Err(e) => {
                // Cleanup partial bundle files on failure
                let final_path = format!("{}/{}", args.dir, filename);
                let _ = std::fs::remove_file(&final_path);
                crate::engine::DownloadBundle::cleanup_bundle(&args.dir, &filename);
                eprintln!("\n  \x1b[31m✖ Download Failed: {}\x1b[0m", e);
                std::process::exit(1);
            }
        }
    }
}
