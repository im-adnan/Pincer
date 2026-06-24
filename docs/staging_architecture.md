# Pincer Download Staging Architecture

The Pincer engine uses a staged download architecture modeled after Safari's native `.download` bundle approach. This design resolves issues with macOS QuickLook and Finder metadata indexing daemons crashing or stuttering when parsing incomplete media containers.

## 1. The `.download` Wrapper Directory

Instead of writing raw bytes directly to the final file name (e.g., `video.mp4`), Pincer creates a wrapper directory: `video.mp4.download`.

This directory mimics an Apple package format and hides the incomplete raw files from Finder's metadata indexing systems.

## 2. Inner File Structure

Inside the `.download` directory, the following files are maintained:

1. **`Info.plist`**: A lightweight Apple property list file. It explicitly tells macOS that this directory is a downloading package. It contains:
   - `DownloadTitle`: The name of the file being downloaded.
   - `DownloadTargetUTI`: The UTI (Uniform Type Identifier) of the file type.
2. **`state.json`**: A decentralized metadata file for the download session. It stores heavy task metadata specific to this download, such as:
   - File URL and headers
   - Thread split configurations
   - Progress arrays (`worker_progress`)
   - Chunk sizes
3. **The payload file**: The raw bytes being written (e.g., `.part` files).

## 3. Decentralized Session State

Previously, Pincer stored all task progress arrays and HTTP headers inside the global `~/.pincer/pincer.session` file, which caused memory bloat as the number of tasks grew.

With the staging architecture, `state.json` inside the `.download` bundle maintains this heavy state. The global `pincer.session` file only acts as a lightweight inventory that points to the `.download` bundles for active tasks. If a user manually deletes a `.download` folder from Finder, Pincer gracefully flags the task as errored upon restart.

## 4. Finalization and Cleanup

Upon successful download completion and hash verification:
1. The inner `.part` payload is renamed and moved out of the `.download` directory to the final destination (e.g., `video.mp4`).
2. The `com.apple.quarantine` extended attribute is stripped from the final file to mark it as safe.
3. The temporary `.download` directory (containing `state.json` and `Info.plist`) is permanently deleted.
