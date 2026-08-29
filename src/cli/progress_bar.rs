//! ANSI terminal progress bar & thread status rendering
//!
//! ### Architectural Overview
//! - **What it does**: Renders an interactive, multi-line ANSI terminal progress bar displaying overall progress, download speed, ETA, and per-worker chunk status.
//! - **How it does**: Formats ANSI escape sequences to clear lines and reposition the cursor, computing progress percentage, ETA in seconds, and dynamic visual indicators per thread.
//! - **Where it comes from**: Called periodically by `cli::DirectDownloader` during active direct CLI downloads when `--log` is enabled.
//! - **Where it leads to**: Flushes formatted ANSI output directly to `std::io::stdout()`.

use super::formatters::{format_bytes, format_duration};
use std::io::Write;

/// Renders a dynamic multi-line progress status to the terminal using ANSI escape codes.
pub struct TerminalProgressBar;

impl TerminalProgressBar {
    /// Renders the current download state:
    /// - Progress bar with unicode blocks (`█` / `░`)
    /// - Percentage completed
    /// - Transferred bytes / Total bytes
    /// - Instantaneous download speed (MB/s)
    /// - Estimated Time of Arrival (ETA)
    /// - Per-thread downloaded byte counters
    pub fn render(completed: u64, total: u64, speed: f64, thread_progress: &[u64]) {
        // Calculate percentage completed
        let percent = if total > 0 {
            (completed as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        // Compute estimated remaining seconds based on current speed
        let eta_secs = if speed > 0.0 && total > completed {
            ((total - completed) as f64 / speed) as u64
        } else {
            0
        };

        // Generate 30-character progress bar graphic
        let width = 30;
        let filled = ((percent / 100.0) * width as f64) as usize;
        let bar = format!(
            "[{}{}]",
            "█".repeat(filled.min(width)),
            "░".repeat(width.saturating_sub(filled))
        );

        // Format per-thread worker progress status line
        let mut thread_status = String::new();
        for (i, p) in thread_progress.iter().enumerate() {
            thread_status.push_str(&format!(" [T{}: {}]", i + 1, format_bytes(*p)));
        }

        // Print formatted 2-line display:
        // \r\x1b[2K: Carriage return and clear line
        // \x1b[1A: Move cursor up 1 line to allow smooth overwriting on next render
        print!("\r\x1b[2K  \x1b[1;36m{}\x1b[0m \x1b[1;32m{:>5.1}%\x1b[0m | \x1b[1;37m{}/{}\x1b[0m | \x1b[1;33m{}/s\x1b[0m | \x1b[1;35mETA: {}\x1b[0m\n\x1b[2K  \x1b[90mThreads:{}\x1b[0m\x1b[1A",
            bar,
            percent,
            format_bytes(completed),
            format_bytes(total),
            format_bytes(speed as u64),
            format_duration(eta_secs),
            thread_status
        );
        let _ = std::io::stdout().flush();
    }
}
