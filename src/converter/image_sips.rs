//! macOS sips image conversion
//!
//! ### Architectural Overview
//! - **What it does**: Performs high-performance raster and vector image format conversions using Apple's built-in `sips` (Scriptable Image Processing System) command line utility.
//! - **How it does**: Validates source/target formats against supported image types (`png`, `jpg`, `jpeg`, `tiff`, `gif`, `bmp`, `heic`, `webp`), builds `sips -s format <target> <input> --out <output>` arguments, and executes asynchronously.
//! - **Where it comes from**: Called by `converter::FormatTranscoder::convert_format()` when image formats are detected.
//! - **Where it leads to**: Outputs converted images directly to the destination path.

use std::path::Path;

/// macOS native image conversion engine wrapping `sips`.
pub struct SipsConverter;

impl SipsConverter {
    /// Supported image file extension formats.
    pub const SUPPORTED_FORMATS: &'static [&'static str] =
        &["png", "jpg", "jpeg", "tiff", "gif", "bmp", "heic", "webp"];

    /// Converts an image file from `src_ext` to `target_ext` using `sips`.
    ///
    /// Executes: `sips -s format <target_fmt> <input_path> --out <dest_path>`
    pub async fn convert(
        src_ext: &str,
        target_ext: &str,
        temp_path: &Path,
        dest_path: &Path,
    ) -> bool {
        if !Self::SUPPORTED_FORMATS.contains(&src_ext)
            || !Self::SUPPORTED_FORMATS.contains(&target_ext)
        {
            return false;
        }

        let sips_format = match target_ext {
            "jpg" | "jpeg" => "jpeg",
            "png" => "png",
            "tiff" => "tiff",
            "gif" => "gif",
            "bmp" => "bmp",
            "heic" => "heic",
            "webp" => "webp",
            _ => return false,
        };

        println!(
            "[CONVERTER] Running: sips -s format {} {:?} --out {:?}",
            sips_format, temp_path, dest_path
        );

        match tokio::process::Command::new("sips")
            .arg("-s")
            .arg("format")
            .arg(sips_format)
            .arg(temp_path)
            .arg("--out")
            .arg(dest_path)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    println!("[CONVERTER] sips conversion succeeded!");
                    true
                } else {
                    eprintln!(
                        "[CONVERTER ERROR] sips failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                eprintln!("[CONVERTER ERROR] Failed to execute sips: {}", e);
                false
            }
        }
    }
}
