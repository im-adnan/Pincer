//! macOS .download staging bundle preparation & cleanup
//!
//! ### Architectural Overview
//! - **What it does**: Creates and cleans up macOS native `.download` bundle packages for active downloads, allowing Finder to display download progress badges and proper icons.
//! - **How it does**: Initializes `<filename>.download/` directory, writes an Apple `Info.plist` with UTType identifiers and source URL metadata, and cleans up the bundle on download completion.
//! - **Where it comes from**: Called by `engine::DownloadTask::start()` during initialization and `manager::TaskPostProcessor` upon completion.
//! - **Where it leads to**: Generates macOS package directories on disk and removes bundle metadata folders upon file promotion.

use crate::manager::removal::TaskRemovalManager;
use std::path::Path;

/// Manages the creation and cleanup of macOS `.download` staging packages.
pub struct DownloadBundle;

impl DownloadBundle {
    /// Maps file extension to Apple Uniform Type Identifier (UTI).
    fn get_uti_for_ext(ext: &str) -> &'static str {
        match ext {
            "mp4" | "m4v" => "public.mpeg-4",
            "mov" => "com.apple.quicktime-movie",
            "mkv" => "org.matroska.mkv",
            "mp3" => "public.mp3",
            "pdf" => "com.adobe.pdf",
            "zip" => "public.zip-archive",
            "dmg" => "com.apple.disk-image-udif",
            "pkg" => "com.apple.installer-package-archive",
            "tar" | "gz" | "tgz" => "org.gnu.gnu-tar-archive",
            _ => "public.data",
        }
    }

    /// Prepares a `.download` bundle package directory and writes `Info.plist`.
    pub fn setup_bundle(save_path: &str, filename: &str, url: &str) -> (String, String) {
        Self::prepare_bundle(save_path, filename, url)
    }

    /// Prepares a `.download` bundle package directory and writes `Info.plist`.
    pub fn prepare_bundle(save_path: &str, filename: &str, url: &str) -> (String, String) {
        let bundle_dir = format!("{}/{}.download", save_path, filename);
        let _ = std::fs::create_dir_all(&bundle_dir);

        let ext = Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let uti = Self::get_uti_for_ext(&ext);

        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.pincer.download</string>
    <key>CFBundleName</key>
    <string>{}</string>
    <key>CFBundleTypeUTI</key>
    <string>{}</string>
    <key>DownloadEntryURL</key>
    <string>{}</string>
</dict>
</plist>"#,
            filename, uti, url
        );

        let plist_path = format!("{}/Info.plist", bundle_dir);
        let _ = std::fs::write(plist_path, plist_content);

        let staged_file_path = format!("{}/{}", bundle_dir, filename);
        let relative_staged_name = format!("{}.download/{}", filename, filename);

        (staged_file_path, relative_staged_name)
    }

    /// Removes the `.download` staging bundle folder after promoting the downloaded file.
    pub fn cleanup_bundle(save_path: &str, filename: &str) {
        if !filename.is_empty() {
            let bundle_dir = format!("{}/{}.download", save_path, filename);
            let path = Path::new(&bundle_dir);
            if path.exists()
                && path.is_dir()
                && path.extension().and_then(|e| e.to_str()) == Some("download")
                && !TaskRemovalManager::is_protected_directory(path)
            {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}
