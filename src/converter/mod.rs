//! Transcoding orchestrator & rollback
//!
//! ### Architectural Overview
//! - **What it does**: Dispatches post-download media conversion tasks to specialized tools (`sips`, `cupsfilter`, `ffmpeg`, `afconvert`) and provides atomic rollback safety.
//! - **How it does**: Creates a temporary copy of the source file, delegates conversion to format-specific sub-converters, and restores the original file if the transcoding process fails or is interrupted.
//! - **Where it comes from**: Called by `manager::TaskPostProcessor::handle_conversion()` and `cli::DirectDownloader::run()`.
//! - **Where it leads to**: Converts local media assets on disk or rolls back without data loss.

pub mod audio_afconvert;
pub mod detector;
pub mod image_sips;
pub mod pdf_cups;
pub mod video_ffmpeg;

pub use audio_afconvert::AfconvertConverter;
pub use detector::FormatDetector;
pub use image_sips::SipsConverter;
pub use pdf_cups::CupsConverter;
pub use video_ffmpeg::FfmpegConverter;

use std::path::Path;

/// Central transcoding coordinator managing format detection, converter routing, and rollback.
pub struct FormatTranscoder;

impl FormatTranscoder {
    /// Attempts to convert a file to a new target format inferred from its destination filename.
    ///
    /// Safety mechanisms:
    /// 1. Backs up original file to a temporary `<file>.tmp_orig` path.
    /// 2. Detects source format (via extension, URL path, or MIME headers).
    /// 3. Routes to appropriate native converter (`sips`, `cupsfilter`, `afconvert`, `ffmpeg`).
    /// 4. If conversion succeeds, deletes the temporary backup file.
    /// 5. If conversion fails, restores the original file atomically from backup.
    pub async fn convert_format(
        file_path_str: &str,
        url: &str,
        file_type: Option<&str>,
    ) -> Result<(), String> {
        let dest_path = Path::new(file_path_str);
        let target_ext = match dest_path.extension().and_then(|e| e.to_str()) {
            Some(ext) => ext.to_lowercase(),
            None => return Ok(()), // No conversion needed if extension is absent
        };

        let src_ext = FormatDetector::detect_src_extension(dest_path, url, file_type);
        if src_ext.is_empty() || src_ext == target_ext {
            return Ok(()); // Source already matches target format
        }

        println!(
            "[CONVERTER] Converting from '{}' to '{}' for file: {:?}",
            src_ext, target_ext, dest_path
        );

        // Step 1: Create backup copy before initiating conversion
        let temp_path_str = format!("{}.tmp_orig", file_path_str);
        let temp_path = Path::new(&temp_path_str);

        if let Err(e) = std::fs::rename(dest_path, temp_path) {
            eprintln!(
                "[CONVERTER ERROR] Failed to create temporary file for conversion: {}",
                e
            );
            return Ok(());
        }

        // Step 2: Route to appropriate specialized converter
        let success = SipsConverter::convert(&src_ext, &target_ext, temp_path, dest_path).await
            || CupsConverter::convert(&src_ext, &target_ext, temp_path, dest_path).await
            || AfconvertConverter::convert(&src_ext, &target_ext, temp_path, dest_path).await
            || FfmpegConverter::convert(temp_path, dest_path).await;

        // Step 3: Cleanup backup or restore on failure
        if success {
            println!("[CONVERTER] Format conversion succeeded!");
            let _ = std::fs::remove_file(temp_path);
        } else {
            eprintln!("[CONVERTER ERROR] All conversion methods failed. Restoring original file.");
            let _ = std::fs::rename(temp_path, dest_path);
        }

        Ok(())
    }
}
