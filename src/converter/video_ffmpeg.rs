//! ffmpeg audio/video conversion
//!
//! ### Architectural Overview
//! - **What it does**: Performs audio and video format transcoding (e.g. `mp4`, `mkv`, `webm`, `mov`, `mp3`, `flac`) using `ffmpeg`.
//! - **How it does**: Executes `ffmpeg -y -i <input> <output>` via `tokio::process::Command` and inspects command exit status and stderr.
//! - **Where it comes from**: Called by `converter::FormatTranscoder::convert_format()` when audio/video conversions are requested.
//! - **Where it leads to**: Transcodes audio and video streams to the requested container format or returns `false` if `ffmpeg` is not installed.

use std::path::Path;

/// Cross-platform audio/video transcoding engine wrapping `ffmpeg`.
pub struct FfmpegConverter;

impl FfmpegConverter {
    /// Transcodes a media file to the destination path's format using `ffmpeg`.
    ///
    /// Executes: `ffmpeg -y -i <input_path> <dest_path>`
    /// - `-y`: Overwrite output files without asking.
    /// - `-i`: Input file path.
    pub async fn convert(temp_path: &Path, dest_path: &Path) -> bool {
        println!(
            "[CONVERTER] Running: ffmpeg -y -i {:?} {:?}",
            temp_path, dest_path
        );

        match tokio::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(temp_path)
            .arg(dest_path)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    println!("[CONVERTER] ffmpeg conversion succeeded!");
                    true
                } else {
                    eprintln!(
                        "[CONVERTER ERROR] ffmpeg failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                println!("[CONVERTER] ffmpeg not available: {}", e);
                false
            }
        }
    }

    /// Merges an audio and video file into a destination file using `ffmpeg`.
    pub async fn merge(video_path: &Path, audio_path: &Path, dest_path: &Path) -> bool {
        println!(
            "[CONVERTER] Running: ffmpeg -y -i {:?} -i {:?} -c:v copy -c:a aac {:?}",
            video_path, audio_path, dest_path
        );

        match tokio::process::Command::new("ffmpeg")
            .arg("-y")
            .arg("-i")
            .arg(video_path)
            .arg("-i")
            .arg(audio_path)
            .arg("-c:v")
            .arg("copy")
            .arg("-c:a")
            .arg("aac")
            .arg(dest_path)
            .output()
            .await
        {
            Ok(output) => {
                if output.status.success() {
                    println!("[CONVERTER] ffmpeg merge succeeded!");
                    true
                } else {
                    eprintln!(
                        "[CONVERTER ERROR] ffmpeg merge failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    false
                }
            }
            Err(e) => {
                println!("[CONVERTER] ffmpeg not available: {}", e);
                false
            }
        }
    }
}
