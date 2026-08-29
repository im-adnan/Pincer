//! Unique filename generation
//!
//! ### Architectural Overview
//! - **What it does**: Generates unique, non-colliding destination filenames for newly added downloads when tasks with identical filenames are already present.
//! - **How it does**: Sanitizes the base filename via `sanitize_filename()`, checks existing task filenames in memory, and incrementally appends `_1`, `_2`, etc. before the file extension.
//! - **Where it comes from**: Called by `manager::TaskSpawner::register_task()`.
//! - **Where it leads to**: Returns a unique destination filename string preventing accidental file overwrite collisions.

use super::state::TaskControl;
use crate::common::sanitize_filename;
use std::collections::HashMap;
use std::path::Path;

/// Generates unique filenames to prevent disk overwrite collisions across concurrent tasks.
pub struct UniqueNameGenerator;

impl UniqueNameGenerator {
    /// Generates a conflict-free filename by appending numeric indexes if another task uses the same name.
    ///
    /// Generation algorithm:
    /// 1. Sanitizes base filename (strips illegal characters).
    /// 2. Checks all registered task filenames in memory (excluding `excluding_gid` if updating).
    /// 3. While a collision exists, appends `_1`, `_2`, `_3` before the file extension.
    /// 4. Returns the verified unique filename.
    pub fn generate_unique(
        filename: &str,
        excluding_gid: Option<&str>,
        tasks: &HashMap<String, TaskControl>,
    ) -> String {
        let sanitized = sanitize_filename(filename);
        let mut unique_name = sanitized.clone();
        let mut counter = 1;

        let path = Path::new(&sanitized);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(&sanitized);
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        while tasks.values().any(|c| {
            if let Some(egid) = excluding_gid {
                if c.status.gid == egid {
                    return false;
                }
            }
            if let Some(file) = c.status.files.first() {
                let existing_filename = Path::new(&file.path)
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");
                if existing_filename == unique_name {
                    return true;
                }
            }
            false
        }) {
            if extension.is_empty() {
                unique_name = format!("{}_{}", stem, counter);
            } else {
                unique_name = format!("{}_{}.{}", stem, counter, extension);
            }
            counter += 1;
        }

        unique_name
    }
}
