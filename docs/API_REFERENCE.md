# Pincer JSON-RPC API Reference

Pincer is a high-performance download engine rewritten in Rust. It provides **1:1 feature parity** with the Aria2 RPC specification, allowing it to serve as a drop-in replacement for tools that expect an Aria2-compatible backend.

## Connection Details
- **Default Port**: `6842`
- **Protocol**: WebSocket (`ws://127.0.0.1:6842/jsonrpc`)
- **Format**: JSON-RPC 2.0

---

## Authentication
If a secret token is configured, it must be passed as the first element of the `params` array.
- **Format**: `["token:your_secret", ...params]`

Example (pin.addUri):
```json
{
  "jsonrpc": "2.0",
  "method": "pin.addUri",
  "id": "1",
  "params": ["token:mysecret", ["https://example.com/file.zip"], {"dir": "/downloads"}]
}
```

---

## Core Features

### 1. Task Management

| Method | Description | Parameters |
| :--- | :--- | :--- |
| `pin.addUri` | Adds a new download task. | `[ [uris], options ]` |
| `pin.addTorrent` | Adds a BitTorrent download (Stub for now). | `[ torrent_file, [uris], options ]` |
| `pin.pause` | Pauses a specific task. | `[ gid ]` |
| `pin.unpause` | Resumes a specific task. | `[ gid ]` |
| `pin.remove` | Stops and removes a task. | `[ gid ]` |
| `pin.pauseAll` | Pauses all active/waiting tasks. | `[]` |
| `pin.unpauseAll` | Resumes all paused/waiting tasks. | `[]` |

### 2. Status & Monitoring

| Method | Description | Response Type |
| :--- | :--- | :--- |
| `pin.tellStatus` | Metadata for a specific task. | `TaskStatus` object |
| `pin.tellActive` | Lists all active tasks. | Array of `TaskStatus` |
| `pin.tellWaiting` | List of waiting/paused tasks. | Array of `TaskStatus` |
| `pin.tellStopped` | List of completed/errored tasks. | Array of `TaskStatus` |
| `pin.getGlobalStat`| Global speed and task counts. | `GlobalStat` object |

### 3. Option Configuration

| Method | Description |
| :--- | :--- |
| `pin.changeOption` | Update options for a specific task. |
| `pin.getOption` | Retrieve options for a specific task. |
| `pin.changeGlobalOption` | Update engine-wide configuration. |
| `pin.getGlobalOption` | Retrieve current global configuration. |

**Common Options (Flags):**
- `dir`: Target directory.
- `out`: Output filename.
- `split`: Connection count (threads).

### 4. Result Management

| Method | Description |
| :--- | :--- |
| `pin.purgeDownloadResult` | Purges all stopped/completed tasks. |
| `pin.removeDownloadResult` | Purges a specific task from history. |

---

## Real-Time Notifications
Pincer streams events automatically over the WebSocket connection.

| Method | Scenario |
| :--- | :--- |
| `pin.onDownloadStart` | Triggered when a task enters **active** state. |
| `pin.onDownloadPause` | Triggered when a task is manually paused. |
| `pin.onDownloadComplete` | Triggered when a task finishes successfully. |
| `pin.onDownloadError` | Triggered when a task fails. |

**Notification Format:**
```json
{
  "jsonrpc": "2.0",
  "method": "pin.onDownloadComplete",
  "params": [{"gid": "..."}]
}
```

---

## CLI Mode
For quick direct download tests without using the RPC server:
```bash
./pincer "https://example.com/file.zip"
```
This will download the file to the current directory using 4 threads by default.

---

## Technical Features (Rust Backend)
- **Multi-Threaded Chunking**: Divides files into segments and downloads them in parallel.
- **Async I/O**: Built on `tokio` for high-throughput, low-latency processing.
- **Range Support Detection**: Automatically falls back to single-thread if server doesn't support `Accept-Ranges`.
- **Zero-Allocation Writes**: Uses `write_at` (on Unix) to write chunks directly to their final positions on disk.
