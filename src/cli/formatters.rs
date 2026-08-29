//! Byte and duration formatting helpers
//!
//! ### Architectural Overview
//! - **What it does**: Provides human-readable string formatting for byte counts (`B`, `KB`, `MB`, `GB`, `TB`) and time durations (`HH:MM:SS` or `MM:SS`).
//! - **How it does**: Performs metric unit division with decimal precision for bytes, and computes hours, minutes, and seconds from raw elapsed/remaining seconds.
//! - **Where it comes from**: Called by CLI presentation modules (`TerminalProgressBar`, `DirectDownloader`) and logging functions.
//! - **Where it leads to**: Produces formatted string slices returned to the caller for terminal output.

/// Formats a raw byte count into a human-friendly string with appropriate units.
///
/// Converts large numbers of bytes into readable representations (e.g. `1048576` -> `"1.00 MB"`).
/// Supports bytes (B), kilobytes (KB), megabytes (MB), gigabytes (GB), and terabytes (TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Formats an elapsed or remaining duration in seconds into `HH:MM:SS` or `MM:SS`.
///
/// Automatically omits the hour segment if duration is under 1 hour for concise terminal output.
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}
