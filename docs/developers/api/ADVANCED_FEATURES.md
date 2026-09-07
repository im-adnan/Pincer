# Advanced Features

Detailed documentation of Pincer's advanced capabilities beyond basic download management.

> **See also**: [Task Management](TASK_MANAGEMENT.md) · [Configuration](CONFIGURATION.md) · [Events](EVENTS.md)

---

## URL Resolution (`pin.resolveUrl`)

Pincer features a powerful URL resolution engine that handles more than just simple redirects.

- **Universal Redirects**: Follows HTTP redirects to find the final CDN link.
- **Metadata Extraction**: Extracts filenames from `Content-Disposition` headers.
- **Platform Scraping**: Automatically identifies and extracts high-quality video links from platforms like **Pexels** by parsing `NEXT_DATA` or meta tags.
- **Resumability Check**: Verifies if the server supports range requests before starting.

---

## Real-Time File Format Conversion

Pincer features an integrated format conversion engine that triggers automatically upon download completion if a different file format is requested.

- **Image Conversion**: Utilizes macOS `sips` for standard image transcoding between formats like `png`, `jpg`/`jpeg`, `webp`, `heic`/`heif`.
  - **PDF Support**: Utilizes `sips` with a automatic built-in fallback to macOS's native `cupsfilter` utility for extremely reliable PDF generation.
- **Audio/Video Conversion**: Uses `ffmpeg` (if globally installed) or falls back to macOS's native `afconvert` utility for audio (`mp3`, `wav`, `m4a`, `aac`).
- **Converting Status**: During the transcoding phase, the task's state changes to `"converting"` before final completion.

---

## Multi-Protocol Support (FTP & SFTP)

Beyond standard HTTP/HTTPS, Pincer Engine fully supports legacy and secure file transfer protocols.

- **FTP (`ftp://`)**: Supports anonymous login natively. Connections seamlessly plug into Pincer's `DownloadWorker` pool for multi-threaded downloads.
- **SFTP (`sftp://`)**: Supports completely secure file transfers over SSH. Handles passwordless authentication automatically by leveraging the user's local SSH agent.

---

## Metalink & Multi-Source Failover

Pincer can parse `.meta4` (Metalink) XML files using the `pin.addMetalink` JSON-RPC method, granting highly resilient downloads.

- **XML Parsing**: Uses safe and fast XML parsing via `quick-xml`. Simply provide the base64-encoded XML document to the RPC method.
- **Multi-Source Failover**: Metalink files often provide multiple fallback URIs for a single file. If Pincer encounters a connection error (e.g. timeout, connection refused) from the `priority 1` server while downloading a specific chunk, the worker will automatically retry that chunk using the `priority 2` fallback server without aborting the task.
- **Checksum Validation**: If the Metalink XML provides a `<hash type="sha-256">` node, Pincer engine automatically computes the SHA-256 hash of the final file on a separate non-blocking thread post-download. If corruption is detected, the task is safely set to an `error` state.

---

## Global Speed Modes

The `speed-mode` option in `pin.changeGlobalOption` allows for high-level bandwidth control:

- `max_bandwidth` (or `max`): No limit applied.
- `half_bandwidth` (or `half`): Limits speed to 50% of the maximum speed seen during the current session.
- `min_bandwidth` (or `min`): Limits speed to a sub-kb/s level (approx. 768 bytes/s) without pausing, keeping connections alive.

---

## Trash Integration

The `pin.removeAndFile` method utilizes the system trash (e.g., macOS Trash) rather than performing a permanent deletion. This provides a safety net for users who may want to recover a deleted download.

---

## Native Safe Restarts (Non-Resumable Tasks)

The engine natively protects against corrupted files when dealing with non-resumable connections. If you invoke `pin.unpause` on a non-resumable task that previously failed or was interrupted, the backend engine automatically resets the task's offset to zero and permanently deletes (`rm -rf`) the partially downloaded corrupt file from the disk before restarting the worker. This handles the complex teardown process instantaneously without requiring multi-step RPC interactions.

---

## Chronological Queue Order (FIFO Scheduling)

Pincer implements strict First-In-First-Out (FIFO) queueing. Every task is tagged with a precise `created_at` Unix millisecond timestamp upon addition. When running multiple downloads under a concurrency limit, the engine strictly schedules and spawns waiting tasks chronologically.
