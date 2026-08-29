//! Task runner & SHA256 integrity checker
//!
//! ### Architectural Overview
//! - **What it does**: Handles post-download verification and finalization including SHA256 checksum validation, promoting staged `.part` files, removing quarantine xattrs, and running format transcoding.
//! - **How it does**: Computes SHA256 hashes inside `tokio::task::spawn_blocking`, renames staged files to destination filenames, cleans up `.download` bundles, and invokes `FormatTranscoder`.
//! - **Where it comes from**: Called by `manager::TaskRunner::run_task()` when worker download streams complete.
//! - **Where it leads to**: Verifies file integrity, transitions task status to `complete`, and triggers `pin.onDownloadComplete` notifications.

use crate::converter::FormatTranscoder;
use crate::engine::DownloadBundle;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Handles post-download verification, checksums, file promotion, and format transcoding.
pub struct TaskPostProcessor;

impl TaskPostProcessor {
    /// Computes and verifies the SHA256 hash of a file against expected checksum.
    ///
    /// Executes hash calculation inside `spawn_blocking` to avoid stalling the async runtime.
    pub async fn verify_sha256(file_path: &str, expected_hash: &str) -> bool {
        let file_path_clone = file_path.to_string();
        let expected_clone = expected_hash.to_string();

        match tokio::task::spawn_blocking(move || {
            let mut file = std::fs::File::open(&file_path_clone)?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut hasher)?;
            let hash = hasher.finalize();
            Ok::<String, std::io::Error>(hex::encode(hash))
        })
        .await
        {
            Ok(Ok(actual_hash)) => {
                if actual_hash.to_lowercase() == expected_clone.to_lowercase() {
                    true
                } else {
                    eprintln!(
                        "Hash mismatch! Expected: {}, Actual: {}",
                        expected_clone, actual_hash
                    );
                    false
                }
            }
            _ => {
                eprintln!("Failed to calculate file hash.");
                false
            }
        }
    }

    /// Promotes staged `.part` payload to destination filename and cleans up bundle directory.
    pub fn promote_and_cleanup(dir: &str, filename: &str, part_filename: &str) {
        let file_path = format!("{}/{}", dir, part_filename);
        let final_path = format!("{}/{}", dir, filename);

        if Path::new(&file_path).exists() && file_path != final_path {
            let _ = std::fs::rename(&file_path, &final_path);
        }

        DownloadBundle::cleanup_bundle(dir, filename);
    }

    /// Strips macOS Gatekeeper quarantine extended attributes from the completed file.
    pub fn remove_quarantine(final_path: &str) {
        #[cfg(target_os = "macos")]
        if Path::new(final_path).exists() {
            let _ = xattr::remove(final_path, "com.apple.quarantine");
        }
    }

    /// Executes format transcoding if requested by the task options or filename extension.
    pub async fn handle_conversion(
        final_path: &str,
        url: &str,
        file_type: Option<&str>,
    ) -> Result<(), String> {
        if !final_path.is_empty() && !url.is_empty() {
            FormatTranscoder::convert_format(final_path, url, file_type).await?;
        }
        Ok(())
    }
}
