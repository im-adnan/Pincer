//! macOS afconvert audio conversion
//!
//! ### Architectural Overview
//! - **What it does**: Performs native audio transcoding (`mp3`, `wav`, `m4a`, `aac`, `flac`) using the macOS built-in `afconvert` utility.
//! - **How it does**: Maps target audio extensions to Apple CoreAudio format specifiers (`mpg3`, `m4af`, `WAVE`) and executes `afconvert -f <format> -d <data> <input> <output>` asynchronously.
//! - **Where it comes from**: Called by `converter::FormatTranscoder::convert_format()` when audio formats are detected.
//! - **Where it leads to**: Produces converted audio files or returns `false` on unsupported audio types.

use std::path::Path;

/// macOS native audio transcoding engine wrapping `afconvert`.
pub struct AfconvertConverter;

impl AfconvertConverter {
    /// Supported audio formats that can be processed by `afconvert`.
    pub const SUPPORTED_FORMATS: &'static [&'static str] = &["mp3", "wav", "m4a", "aac", "flac"];

    /// Converts an audio file from `src_ext` to `target_ext` using `afconvert`.
    ///
    /// Maps target formats to CoreAudio format identifiers:
    /// - `"mp3"` -> format: `"mpg3"`, data: `"wha?"`
    /// - `"m4a"` / `"aac"` -> format: `"m4af"`, data: `"aac "`
    /// - `"wav"` -> format: `"WAVE"`, data: `"LEI16"` (Little-Endian 16-bit PCM)
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

        let (af_format, af_data) = match target_ext {
            "mp3" => ("mpg3", "wha?"),
            "m4a" | "aac" => ("m4af", "aac "),
            "wav" => ("WAVE", "LEI16"),
            _ => ("", ""),
        };

        if af_format.is_empty() {
            return false;
        }

        println!(
            "[CONVERTER] Running: afconvert -f {} -d {:?} {:?} {:?}",
            af_format, af_data, temp_path, dest_path
        );

        let mut cmd = tokio::process::Command::new("afconvert");
        cmd.arg("-f").arg(af_format);
        if af_data != "wha?" {
            cmd.arg("-d").arg(af_data);
        }
        cmd.arg(temp_path).arg(dest_path);

        match cmd.output().await {
            Ok(output) => {
                if output.status.success() {
                    println!("[CONVERTER] afconvert conversion succeeded!");
                    true
                } else {
                    eprintln!(
                        "[CONVERTER ERROR] afconvert failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                eprintln!("[CONVERTER ERROR] Failed to execute afconvert: {}", e);
                false
            }
        }
    }
}
