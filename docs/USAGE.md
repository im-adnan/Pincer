# Pincer Comprehensive Manual & API Reference

> **Related Documentation**:
> * If you are looking to contribute code to Pincer, start with the [**Contributing Guide**](CONTRIBUTING.md).
> * To see our long-term architectural plans, check out the [**Future Roadmap**](FUTURE.md).

This document consolidates all information regarding **Pincer** — a high-performance download engine written entirely in Rust from the ground up — including its configuration, JSON-RPC interface, and feature set. It serves as the single source of truth for the project.

---

## 1. Command Line Interface (CLI) & Thread Control

### Thread Control in Pincer
In **Pincer**, download threads are dynamically controlled **via the JSON-RPC interface** when adding or modifying a task using options like `split` (which defines the number of connections/threads per download) and `min-split-size`[cite: 1].

### CLI Mode (Direct Download)
Pincer now features a professional CLI powered by `lexopt`, allowing it to be used as a standalone tool.

#### Basic Usage
For quick direct download tests without using the RPC server:
```bash
./target/release/pincer "https://example.com/file.zip"
```
This will download the file to the current directory using multiple threads by default.

#### Advanced Arguments
| Flag | Long Form | Description | Default |
| :--- | :--- | :--- | :--- |
| `-s` | `--split` | Number of concurrent threads/connections (1-99). | `1` |
| `-d` | `--dir` | Target directory for the download. | Current Dir |
| `-o` | `--out` | Custom output filename. | From URL |
| `-f` | `--format`| Target format to convert the downloaded file to. | N/A |
| `-l` | `--log` | Enable detailed logging. | `false` |
| `-p` | `--port` | RPC server port. | `6842` |
| `-D` | `--daemon`| Run Pincer Engine as a detached background daemon. | `false` |
| `-h` | `--help` | Print help information. | N/A |

## Roadmap
The CLI parser (using `lexopt`) will be expanded to support additional flags for users who want to use Pincer exclusively from the terminal without the RPC server:
*   `-v` or `--version`
*   `-V` or `--check-integrity`
*   `-j` or `--max-concurrent-downloads`
*   `-c` or `--continue` (Resumption)


**Example (High-Speed 16-Thread Download):**
```bash
./pincer "https://example.com/movie.mp4" -s 16 -d ~/Downloads -o holiday_video.mp4
```

### Technical Architecture & Threads
*   **Rust-Native Performance**: Built on `tokio` for non-blocking I/O and zero-allocation disk writes.
*   **Dynamic Thread Scaling**: Supports up to **99 threads** per task. The engine automatically splits the file into equal byte-ranges and assigns a dedicated worker to each.
*   **Range Support Detection**: If a server does not support `Accept-Ranges`, Pincer gracefully falls back to a single thread to ensure data integrity.

---

## 2. Session Persistence & Resumption

Pincer now includes a built-in persistence layer. All tasks, global options, and progress are saved to a session file.

- **Session File**: `pincer.session` (JSON format).
- **Auto-Save**: Triggered on every significant state change (adding a task, pausing, or changing options).
- **Auto-Load**: On startup, Pincer detects the session file and restores all tasks.
- **Resumption**: Interrupted downloads will automatically resume from the last successfully written byte using HTTP Range requests.

---

## 3. Connection & Authentication

- **Default Port**: `6842` (Configurable via `-p` / `--port` CLI option)
- **WebSocket URL**: `ws://127.0.0.1:<PORT>/jsonrpc`
- **Protocol**: JSON-RPC 2.0

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. It implements a robust, proprietary JSON-RPC interface for full engine control.

### Authentication
If an RPC secret is configured, pass it as the **first element** of the `params` array in the format `"token:YOUR_SECRET"`.

**Example** (`pin.addUri`):
```json
{
  "jsonrpc": "2.0",
  "method": "pin.addUri",
  "id": "1",
  "params": [
    "token:mysecret",
    ["https://example.com/file.zip"],
    {"dir": "/downloads", "split": 8}
  ]
}
```

---

## 4. Configuration & Default Settings

### Default Engine Configuration (`pincer.conf` style)[cite: 1, 5]

**RPC**:
- `enable-rpc=true`
- `rpc-allow-origin-all=true`
- `rpc-listen-all=true`

**File System**:
- `auto-save-interval=10`
- `disk-cache=64M`
- `file-allocation=none`
- `no-file-allocation-limit=64M`
- `save-session-interval=10`

**Task Parameters**:
- `check-certificate=false`
- `max-file-not-found=10`
- `max-tries=0`
- `retry-wait=10`
- `connect-timeout=10`
- `timeout=10`
- `min-split-size=1M`
- `http-accept-gzip=true`
- `remote-time=true`
- `summary-interval=0`
- `content-disposition-default-utf8=true`

**BitTorrent Parameters** (for future support):
- `bt-detach-seed-only=true`
- `bt-enable-lpd=true`
- `bt-hash-check-seed=true`
- `bt-max-peers=128`
- `bt-prioritize-piece=head`
- `bt-remove-unselected-file=true`
- `bt-seed-unverified=false`
- `bt-tracker-connect-timeout=10`
- `bt-tracker-timeout=10`
- `enable-dht=true`
- `enable-dht6=true`
- `enable-peer-exchange=true`
- `dht-entry-point=dht.transmissionbt.com:6881`
- `dht-entry-point6=dht.transmissionbt.com:6881`
- `peer-agent=Transmission/3.00`
- `peer-id-prefix=-TR3000-`

**Keys Requiring Engine Restart**:
`rpc-listen-port`, `rpc-secret`, `listen-port`, `dht-listen-port`

### Common Task Options
- `dir` — Target directory
- `out` — Output filename
- `split` — Number of connections/threads
- `max-connection-per-server` — Max connections per server
- `min-split-size` — Minimum split size (e.g. `1M`)
- `max-download-limit` — Speed limit per download
- `header` — Custom HTTP headers
- `user-agent` — Custom User-Agent

---

### User Preferences (UI / Application Level)
These keys are tracked by the frontend and used for application-level state:
`auto-check-update`, `auto-hide-window`, `auto-sync-tracker`, `cookie`, `enable-upnp`, `engine-bin-path`, `engine-max-connection-per-server`, `favorite-directories`, `hide-app-menu`, `history-directories`, `keep-seeding`, `keep-window-state`, `last-check-update-time`, `last-sync-tracker-time`, `locale`, `log-level`, `new-task-show-downloading`, `no-confirm-before-delete-task`, `open-at-login`, `protocols`, `proxy`, `resume-all-when-app-launched`, `run-mode`, `show-progress-bar`, `task-notification`, `theme`, `tracker-source`, `tray-speedometer`.

---
### System Keys (Global/Session Level)
These keys map directly to global or task option updates (`pin.changeGlobalOption` or `pin.changeOption`):

`all-proxy-passwd`, `all-proxy-user`, `all-proxy`, `allow-overwrite`, `allow-piece-length-change`, `always-resume`, `async-dns`, `auto-file-renaming`, `bt-enable-hook-after-hash-check`, `bt-enable-lpd`, `bt-exclude-tracker`, `bt-external-ip`, `bt-force-encryption`, `bt-hash-check-seed`, `bt-load-saved-metadata`, `bt-max-peers`, `bt-metadata-only`, `bt-min-crypto-level`, `bt-prioritize-piece`, `bt-remove-unselected-file`, `bt-request-peer-speed-limit`, `bt-require-crypto`, `bt-save-metadata`, `bt-seed-unverified`, `bt-stop-timeout`, `bt-tracker-connect-timeout`, `bt-tracker-interval`, `bt-tracker-timeout`, `bt-tracker`, `check-integrity`, `checksum`, `conditional-get`, `connect-timeout`, `content-disposition-default-utf8`, `continue`, `dht-file-path`, `dht-file-path6`, `dht-listen-port`, `dir`, `dry-run`, `enable-http-keep-alive`, `enable-http-pipelining`, `enable-mmap`, `enable-peer-exchange`, `file-allocation`, `follow-metalink`, `follow-torrent`, `force-save`, `force-sequential`, `ftp-passwd`, `ftp-pasv`, `ftp-proxy-passwd`, `ftp-proxy-user`, `ftp-proxy`, `ftp-reuse-connection`, `ftp-type`, `ftp-user`, `gid`, `hash-check-only`, `header`, `http-accept-gzip`, `http-auth-challenge`, `http-no-cache`, `http-passwd`, `http-proxy-passwd`, `http-proxy-user`, `http-proxy`, `http-user`, `https-proxy-passwd`, `https-proxy-user`, `https-proxy`, `index-out`, `listen-port`, `lowest-speed-limit`, `max-concurrent-downloads`, `max-connection-per-server`, `max-download-limit`, `max-file-not-found`, `max-mmap-limit`, `max-overall-download-limit`, `max-overall-upload-limit`, `max-resume-failure-tries`, `max-tries`, `max-upload-limit`, `metalink-base-uri`, `metalink-enable-unique-protocol`, `metalink-language`, `metalink-location`, `metalink-os`, `metalink-preferred-protocol`, `metalink-version`, `min-split-size`, `no-file-allocation-limit`, `no-netrc`, `no-proxy`, `no-want-digest-header`, `out`, `parameterized-uri`, `pause-metadata`, `pause`, `piece-length`, `proxy-method`, `realtime-chunk-checksum`, `referer`, `remote-time`, `remove-control-file`, `retry-wait`, `reuse-uri`, `rpc-listen-port`, `rpc-save-upload-metadata`, `rpc-secret`, `seed-ratio`, `seed-time`, `select-file`, `split`, `ssh-host-key-md`, `stream-piece-selector`, `timeout`, `uri-selector`, `use-head`, `user-agent`[cite: 1, 5].

---
> **Note**: Changes to `rpc-listen-port`, `rpc-secret`, `listen-port`, and `dht-listen-port` require an engine restart to take effect[cite: 5].

---

## 4. JSON-RPC Commands Reference

The namespace in Pincer is `pin.*`. Authentication is handled by passing `"token:YOUR_SECRET"` as the first element of the `params` array[cite: 1, 3, 4].

*   **Default Port**: `6842`[cite: 2, 3]
*   **WebSocket URL**: `ws://127.0.0.1:6842/jsonrpc`[cite: 2, 3]
All methods use the `pin.*` namespace.

### Task Management

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.addUri` | Adds a new download task from one or more URIs. | `[uris (Array of Strings), options (Object, Optional), position (Integer, Optional)]` | `gid` (String) |
| `pin.addMetalink` | Adds a download by providing a base64-encoded Metalink XML string. | `[metalink (Base64 String), options?, position?]` | `gid` (String) |
| `pin.addTorrent` | Adds a BitTorrent download by uploading a ".torrent" file. | `[torrent (Base64 String), uris (Array of Strings, Optional), options?, position?]` | `gid` (String) |
| `pin.remove` | Removes the download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.removeAndFile` | Removes the download and moves its file to Trash. | `[gid (String)]` | `true` (Boolean) |
| `pin.forceRemove` | Immediately removes the download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.pause` | Pauses the active/waiting download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.unpause` | Unpauses the paused download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.pauseAll` | Pauses all active/waiting downloads. | `[]` | `OK` (String) |
| `pin.unpauseAll` | Unpauses all paused downloads. | `[]` | `OK` (String) |
| `pin.resolveUrl` | Resolves a URL to its final direct download link. | `[url (String)]` | `ResolveResponse` (Object) |

### Status & Monitoring

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.tellStatus` | Returns progress and status metadata. | `[gid (String), keys (Array of Strings, Optional)]` | `TaskStatus` (Object) |
| `pin.tellActive` | Returns a list of all currently active downloads. | `[keys (Array of Strings, Optional)]` | Array of `TaskStatus` |
| `pin.tellWaiting` | Returns a list of waiting/paused downloads. | `[offset (Int), num (Int), keys (Array, Optional)]` | Array of `TaskStatus` |
| `pin.tellStopped` | Returns a list of stopped downloads. | `[offset (Int), num (Int), keys (Array, Optional)]` | Array of `TaskStatus` |
| `pin.getGlobalStat` | Returns global statistics of the engine. | `[]` | `GlobalStat` (Object) |

### Configuration & Options

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.changeOption` | Changes options of the download dynamically. | `[gid (String), options (Object)]` | `OK` (String) |
| `pin.getOption` | Returns options of the specific download. | `[gid (String)]` | `struct` (Object) |
| `pin.changeGlobalOption` | Changes global options dynamically. | `[options (Object)]` | `OK` (String) |
| `pin.getGlobalOption` | Returns current global options. | `[]` | `struct` (Object) |

### Advanced Features

#### URL Resolution (`pin.resolveUrl`)
Pincer features a powerful URL resolution engine that handles more than just simple redirects.
- **Universal Redirects**: Follows HTTP redirects to find the final CDN link.
- **Metadata Extraction**: Extracts filenames from `Content-Disposition` headers.
- **Platform Scraping**: Automatically identifies and extracts high-quality video links from platforms like **Pexels** by parsing `NEXT_DATA` or meta tags.
- **Resumability Check**: Verifies if the server supports range requests before starting.

#### Real-Time File Format Conversion
Pincer features an integrated format conversion engine that triggers automatically upon download completion if a different file format is requested.
- **Image Conversion**: Utilizes macOS `sips` for standard image transcoding between formats like `png`, `jpg`/`jpeg`, `webp`, `heic`/`heif`.
  - **PDF Support**: Utilizes `sips` with a automatic built-in fallback to macOS's native `cupsfilter` utility for extremely reliable PDF generation.
- **Audio/Video Conversion**: Uses `ffmpeg` (if globally installed) or falls back to macOS's native `afconvert` utility for audio (`mp3`, `wav`, `m4a`, `aac`).
- **Converting Status**: During the transcoding phase, the task's state changes to `"converting"` before final completion.

#### Multi-Protocol Support (FTP & SFTP)
Beyond standard HTTP/HTTPS, Pincer Engine fully supports legacy and secure file transfer protocols.
- **FTP (`ftp://`)**: Supports anonymous login natively. Connections seamlessly plug into Pincer's `DownloadWorker` pool for multi-threaded downloads.
- **SFTP (`sftp://`)**: Supports completely secure file transfers over SSH. Handles passwordless authentication automatically by leveraging the user's local SSH agent.

#### Metalink & Multi-Source Fallover
Pincer can parse `.meta4` (Metalink) XML files using the `pin.addMetalink` JSON-RPC method, granting highly resilient downloads.
- **XML Parsing**: Uses safe and fast XML parsing via `quick-xml`. Simply provide the base64-encoded XML document to the RPC method.
- **Multi-Source Failover**: Metalink files often provide multiple fallback URIs for a single file. If Pincer encounters a connection error (e.g. timeout, connection refused) from the `priority 1` server while downloading a specific chunk, the worker will automatically retry that chunk using the `priority 2` fallback server without aborting the task.
- **Checksum Validation**: If the Metalink XML provides a `<hash type="sha-256">` node, Pincer engine automatically computes the SHA-256 hash of the final file on a separate non-blocking thread post-download. If corruption is detected, the task is safely set to an `error` state.

#### Global Speed Modes
The `speed-mode` option in `pin.changeGlobalOption` allows for high-level bandwidth control:
- `max_bandwidth` (or `max`): No limit applied.
- `half_bandwidth` (or `half`): Limits speed to 50% of the maximum speed seen during the current session.
- `min_bandwidth` (or `min`): Limits speed to a sub-kb/s level (approx. 768 bytes/s) without pausing, keeping connections alive.

#### Trash Integration
The `pin.removeAndFile` method utilizes the system trash (e.g., macOS Trash) rather than performing a permanent deletion. This provides a safety net for users who may want to recover a deleted download.

#### Native Safe Restarts (Non-Resumable Tasks)
The engine natively protects against corrupted files when dealing with non-resumable connections. If you invoke `pin.unpause` on a non-resumable task that previously failed or was interrupted, the backend engine automatically resets the task's offset to zero and permanently deletes (`rm -rf`) the partially downloaded corrupt file from the disk before restarting the worker. This handles the complex teardown process instantaneously without requiring multi-step RPC interactions.

#### Chronological Queue Order (FIFO Scheduling)
Pincer implements strict First-In-First-Out (FIFO) queueing. Every task is tagged with a precise `created_at` Unix millisecond timestamp upon addition. When running multiple downloads under a concurrency limit, the engine strictly schedules and spawns waiting tasks chronologically.

### History Management

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.purgeDownloadResult` | Purges completed/error/removed downloads. | `[]` | `OK` (String) |
| `pin.removeDownloadResult` | Removes a specific task from memory. | `[gid (String)]` | `OK` (String) |

### Key Task Options
When passing an `options` object to `pin.addUri` or `pin.changeOption`, the following keys are used:
- `dir`: Target directory to store the file.
- `out`: The file name of the downloaded file.
- `split` or `-s`: (Integer) Number of connections to use (Default: 5).
- `max-connection-per-server` or `-x`: (Integer) Max connections to a single server (Default: 1).
- `min-split-size` or `-k`: (String) Minimum size to split a chunk (e.g., `1M`).
- `max-download-limit`: (String) Speed limit for the download (e.g., `50K`, `0` for unlimited).
- `header`: (Array of Strings) Custom HTTP Headers.
- `user-agent`: (String) Custom User-Agent string.

### Real-Time Events (WebSocket Notifications)

The engine automatically pushes notifications to connected clients:

| Event | Scenario |
| :--- | :--- |
| `pin.onDownloadStart` | Triggered when a task enters **active** state. |
| `pin.onDownloadPause` | Triggered when a task is manually paused. |
| `pin.onDownloadComplete` | Triggered when a task finishes successfully. |
| `pin.onDownloadError` | Triggered when a task fails. |

**Notification Example**:
```json
{
  "jsonrpc": "2.0",
  "method": "pin.onDownloadComplete",
  "params": [{"gid": "2089b05ecca3d829"}]
}
## 5. API Methodology Comparison

Below is a tracking table showing how Pincer's original API methods map to standard download management patterns.

### Pincer Exclusive Features
*   **Native Memory Safety**: Written entirely in Rust, preventing the segfaults and memory leaks possible in other C-based engines[cite: 1].
*   **Async I/O Worker Pool**: Uses a highly concurrent `tokio` pattern for zero-allocation disk writes, offering lower CPU overhead on high-speed connections[cite: 1, 3].
*   **First-Class WebSocket Layer**: Powered by `axum` for modern, efficient communication[cite: 1, 3].

### Task Addition & Management

| Standard Method Name      | Pincer Method Name    | Status                  |
|---------------------------|-----------------------|-------------------------|
| `addUri`                  | `pin.addUri`          | ✅ Fully Covered        |
| `addTorrent`              | `pin.addTorrent`      | ⚠️ Stubbed              |
| `addMetalink`             | `pin.addMetalink`     | ✅ Fully Covered        |
| `remove`                  | `pin.remove`          | ✅ Fully Covered        |
| `forceRemove`       | `pin.forceRemove`     | ✅ Fully Covered        |
| `pause`             | `pin.pause`           | ✅ Fully Covered        |
| `pauseAll`          | `pin.pauseAll`        | ✅ Fully Covered        |
| `forcePause`        | -                     | ❌ To Be Added          |
| `forcePauseAll`     | -                     | ❌ To Be Added          |
| `unpause`           | `pin.unpause`         | ✅ Fully Covered        |
| `unpauseAll`        | `pin.unpauseAll`      | ✅ Fully Covered        |
| `changePosition`    | -                     | ❌ To Be Added          |
| `changeUri`         | -                     | ❌ To Be Added          |

### Status & Monitoring

| Function             | Pincer Equivalent    | Status           |
|----------------------|----------------------|------------------|
| `tellStatus`   | `pin.tellStatus`     | ✅ Fully Covered |
| `tellActive`   | `pin.tellActive`     | ✅ Fully Covered |
| `tellWaiting`  | `pin.tellWaiting`    | ✅ Fully Covered |
| `tellStopped`  | `pin.tellStopped`    | ✅ Fully Covered |
| `getGlobalStat`| `pin.getGlobalStat`  | ✅ Fully Covered |
| `getUris`      | -                    | ❌ To Be Added   |
| `getFiles`     | -                    | ❌ To Be Added   |
| `getPeers`     | -                    | ❌ To Be Added   |
| `getServers`   | -                    | ❌ To Be Added   |

### Configuration, History & System

| Function                      | Pincer Equivalent           | Status           |
|-------------------------------|-----------------------------|------------------|
| `changeOption`          | `pin.changeOption`          | ✅ Fully Covered |
| `getOption`             | `pin.getOption`             | ✅ Fully Covered |
| `changeGlobalOption`    | `pin.changeGlobalOption`    | ✅ Fully Covered |
| `getGlobalOption`       | `pin.getGlobalOption`       | ✅ Fully Covered |
| `purgeDownloadResult`   | `pin.purgeDownloadResult`   | ✅ Fully Covered |
| `removeDownloadResult`  | `pin.removeDownloadResult`  | ✅ Fully Covered |
| `getVersion`            | `pin.getVersion`            | ✅ Fully Covered |
| `getSessionInfo`        | -                           | ❌ To Be Added   |
| `shutdown`              | `pin.shutdown`              | ✅ Fully Covered |
| `forceShutdown`         | -                           | ❌ To Be Added   |
| `saveSession`           | `pin.saveSession`           | ✅ Fully Covered |
| `system.multicall`            | -                           | ❌ To Be Added   |
| `system.listMethods`          | -                           | ❌ To Be Added   |
| `system.listNotifications`    | -                           | ❌ To Be Added   |

### Events

| Event                        | Pincer Equivalent         | Status           |
|------------------------------|---------------------------|------------------|
| `onDownloadStart`      | `pin.onDownloadStart`     | ✅ Fully Covered |
| `onDownloadPause`      | `pin.onDownloadPause`     | ✅ Fully Covered |
| `onDownloadComplete`   | `pin.onDownloadComplete`  | ✅ Fully Covered |
| `onDownloadError`      | `pin.onDownloadError`     | ✅ Fully Covered |
| `onDownloadStop`       | -                         | ❌ To Be Added   |
| `onBtDownloadComplete` | -                         | ❌ To Be Added   |

---

---

## 7. Verification & Worker Logging

To verify that Pincer is correctly utilizing multi-threading, you can observe the internal worker logs during a CLI download.

**Example Log Output (`-s 16`):**
```text
🚀 Pincer CLI Mode
URL: https://images.pexels.com/.../photo.jpeg
Threads: 16
-------------------------------------------
[INFO] [Worker 0] Assigned range: 0 - 47938 (47939 bytes)
[INFO] [Worker 1] Assigned range: 47939 - 95877 (47939 bytes)
...
[INFO] [Worker 0] Connection established. Segment range: 0 - 47938
[INFO] [Worker 2] Connection established. Segment range: 95878 - 143816
...
[========================================] 100.00%
✅ Download complete!
```

### Why use these features?
1.  **99 Threads**: Maximizes bandwidth utilization on high-latency connections or from servers that throttle single-connection speeds.
2.  **Session Persistence**: Critical for long-running downloads; protects progress against system crashes or engine restarts.
3.  **Advanced CLI**: Allows developers and power users to use Pincer as a drop-in, high-performance download engine.
4.  **Worker Logs**: Provides transparency and allows users to debug connection issues or verify server range support in real-time.

---

## 8. Integration Guide for UI Developers (macOS Swift)

Pincer Engine is designed to run silently as a backend daemon for frontend graphical user interfaces, such as macOS applications built with Swift/SwiftUI. 

To integrate Pincer Engine's capabilities (like FTP/SFTP support, multi-source Metalink fallback, auto-format conversion, etc.) into your UI application, follow this standardized 4-step architecture:

### Step 1: Spawn the Engine as a Background Daemon
Bundle the compiled `pincer` binary inside your macOS App bundle. When your app launches, use `Process` (NSTask) to spawn the engine in daemon mode (`-D`) on a specific port.

```swift
let task = Process()
task.executableURL = Bundle.main.url(forResource: "pincer", withExtension: nil)
// Enable RPC daemon on port 6800, using the background detached flag
task.arguments = ["--enable-rpc=true", "--rpc-listen-port=6800", "-D"]
try? task.run()
```

### Step 2: Connect to the WebSocket RPC Server
Once spawned, Pincer runs a lightweight WebSocket server locally. Establish a persistent WebSocket connection from your Swift app to send commands and receive real-time updates.

```swift
let url = URL(string: "ws://localhost:6800/jsonrpc")!
let session = URLSession(configuration: .default)
let webSocketTask = session.webSocketTask(with: url)
webSocketTask.resume()
```

### Step 3: Trigger Features via JSON-RPC
Instead of executing complex CLI commands, your UI will trigger Pincer's features by sending standardized JSON-RPC payloads over the WebSocket. 

All of Pincer's features are invoked similarly. For example, to trigger the Metalink multi-source download feature:

```swift
// Example: Triggering a Metalink Task
let payload: [String: Any] = [
    "jsonrpc": "2.0",
    "id": UUID().uuidString,
    "method": "pin.addMetalink",
    "params": [
        "PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0i...", // Base64 encoded payload
        ["dir": "/Users/Shared/Downloads", "split": "16"] // Engine options
    ]
]
let jsonData = try! JSONSerialization.data(withJSONObject: payload)
webSocketTask.send(.data(jsonData)) { error in ... }
```
*(Whether you are using `pin.addUri` for FTP links, or `pin.changeGlobalOption` to throttle speeds, the JSON-RPC interface remains identical).*

### Step 4: Map Introspection Models and Listen to Events
Pincer streams real-time status events back to your WebSocket. You do not need to poll manually. Create Swift `Codable` structs that map to Pincer's introspection models (e.g., `PincerFile`, `PincerUri`, `TaskStatus`) to cleanly decode these incoming JSON payloads.

```swift
// Ensure you catch Pincer's standardized events to update your UI:
// - pin.onDownloadStart
// - pin.onDownloadComplete
// - pin.onDownloadError (e.g. emitted if a checksum validation fails!)

webSocketTask.receive { result in
    switch result {
    case .success(.string(let text)):
        // Decode the JSON-RPC response and update your UI progress bars
        let response = try? JSONDecoder().decode(PincerRPCResponse.self, from: text.data(using: .utf8)!)
    case .failure(let error):
        print("WebSocket Error: \(error)")
    }
}
```

By keeping the heavy lifting inside the Pincer Rust engine, your Swift frontend can remain incredibly lightweight—simply sending JSON-RPC commands and painting the UI based on the incoming WebSocket event stream.
