//! Filename sanitization & path safety
//!
//! ### Architectural Overview
//! - **What it does**: Strips directory components and illegal filesystem characters from untrusted filenames to prevent directory traversal attacks and invalid path errors.
//! - **How it does**: Extracts the base filename component using `std::path::Path`, replaces invalid characters (`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, `\0`) with underscores, trims leading dots, and falls back to `download.bin` if empty.
//! - **Where it comes from**: Called by task spawners, CLI orchestrators, unique name generators, and HTTP resolvers.
//! - **Where it leads to**: Returns a sanitized filename string safe for local disk open/create operations.

use std::path::Path;

/// Strips directory components and illegal filesystem characters to prevent path traversal attacks.
///
/// Ensures filenames provided by untrusted remote servers (e.g. via `Content-Disposition`)
/// or user input cannot escape the target download directory:
/// 1. Strips leading path elements (`../`, `../../etc/passwd` -> `passwd`).
/// 2. Replaces illegal OS characters (`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, `\0`) with `_`.
/// 3. Trims leading and trailing dots to prevent hidden or root path creation.
/// 4. Defaults to `"download.bin"` if the resulting string is empty.
pub fn sanitize_filename(filename: &str) -> String {
    // Step 1: Extract base filename component, dropping directory components
    let base_name = Path::new(filename)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| filename.to_string());

    // Step 2: Replace forbidden characters across POSIX / Windows filesystems
    let sanitized: String = base_name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            _ => c,
        })
        .collect();

    // Step 3: Trim surrounding dots and whitespace
    let trimmed = sanitized.trim_matches('.').trim();

    // Step 4: Fall back to safe default if empty
    if trimmed.is_empty() {
        "download.bin".to_string()
    } else {
        trimmed.to_string()
    }
}
