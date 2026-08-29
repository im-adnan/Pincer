//! macOS cupsfilter PDF conversion
//!
//! ### Architectural Overview
//! - **What it does**: Converts images and text documents directly to PDF format using the native macOS `cupsfilter` printing filter utility.
//! - **How it does**: Invokes `cupsfilter -m application/pdf <input> > <output>` asynchronously and redirects standard output to the target PDF file.
//! - **Where it comes from**: Called by `converter::FormatTranscoder::convert_format()` when the target format is `pdf`.
//! - **Where it leads to**: Generates a standard PDF file on disk.

use std::path::Path;

/// macOS native PDF conversion engine wrapping `cupsfilter`.
pub struct CupsConverter;

impl CupsConverter {
    /// Supported input formats that `cupsfilter` can convert to PDF.
    pub const SUPPORTED_SRC: &'static [&'static str] =
        &["png", "jpg", "jpeg", "tiff", "gif", "txt"];

    /// Converts an image or text file to PDF using `cupsfilter`.
    ///
    /// Executes: `cupsfilter -m application/pdf <input_path> > <dest_path>`
    pub async fn convert(
        src_ext: &str,
        target_ext: &str,
        temp_path: &Path,
        dest_path: &Path,
    ) -> bool {
        if target_ext != "pdf" || !Self::SUPPORTED_SRC.contains(&src_ext) {
            return false;
        }

        println!(
            "[CONVERTER] Running: cupsfilter -m application/pdf {:?} > {:?}",
            temp_path, dest_path
        );

        match tokio::process::Command::new("cupsfilter")
            .arg("-m")
            .arg("application/pdf")
            .arg(temp_path)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() && !output.stdout.is_empty() {
                    if let Err(e) = std::fs::write(dest_path, &output.stdout) {
                        eprintln!("[CONVERTER ERROR] Failed to write PDF output: {}", e);
                        false
                    } else {
                        println!("[CONVERTER] cupsfilter conversion succeeded!");
                        true
                    }
                } else {
                    eprintln!(
                        "[CONVERTER ERROR] cupsfilter failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                eprintln!("[CONVERTER ERROR] Failed to execute cupsfilter: {}", e);
                false
            }
        }
    }
}
