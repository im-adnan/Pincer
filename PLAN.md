# Pincer: Rust Download Engine Plan

Pincer is a high-performance, multithreaded download engine written in Rust, designed as a direct drop-in replacement for your existing Aria2-based backend in Sluice.

## 1. Scope & Objective

The goal is to develop a robust HTTP/HTTPS download engine in Rust that communicates over WebSockets using JSON-RPC, mimicking the data structures Sluice currently expects but substituting `aria2.` for the `pin.` namespace.

**Core Capabilities Focus**:
- Regular HTTP/HTTPS downloads.
- Segmented/chunked multithreaded downloading to maximize bandwidth.
- Background execution with robust task pausing, resuming, and removing.
- Dynamic fallback (switching from multithreaded to single-thread if the server doesn't support `Accept-Ranges: bytes`).
- Native Posix-style pre-allocation and concurrent file writing.

**Out of Scope**:
- Torrent (BitTorrent) downloading and peer-to-peer logic.
- Metalinks.

---

## 2. System Architecture

The application is structured into three main concurrency layers running on the `tokio` runtime:

### A. The RPC Server
We will use **axum** combined with **tokio-tungstenite** to host a lightweight WebSocket server on a dedicated thread.
- **Protocol**: JSON-RPC over WebSocket.
- **Lifecycle**: It listens to incoming connections, handles `pin.*` method calls, and broadcasts events like `pin.onDownloadComplete` or `pin.onDownloadStart` via async channels.

### B. The Download Manager
A thread-safe singleton (e.g., `Arc<RwLock<HashMap<String, DownloadTask>>>`) that maintains the global state for all downloads.
- Replaces Aria2's 16-character hex "GID" with simple string identifiers (e.g., UUIDs or auto-incremented integers).
- Tracks global statistics (download and upload speeds).
- Processes `.pause()`, `.resume()`, and `.remove()` logic without blocking the event loop.

### C. The Worker Pool (The "Axel" Pattern)
When a new URL is submitted via `pin.addUri`:
1. **Discovery (HEAD)**: It queries the server to determine `Content-Length` and whether `Accept-Ranges: bytes` is present.
2. **Allocation**: It immediately reserves the file block on the disk using `std::fs::File::set_len`.
3. **Partitioning**: If the server supports chunking, it divides the length by `N` connections and spawns `N` independent async `tokio` tasks (workers).
4. **Sparse Writing**: Each worker uses the async `reqwest::Client` to stream HTTP chunks and writes directly into its assigned byte offset space utilizing `std::os::unix::fs::FileExt::write_at` (bypassing the need to concatenate multiple temp files later).

---

## 3. The JSON-RPC API Interface

We implement exact equivalents to your current `Aria2RPCClient` methods out of the box.

### Task Management
* `pin.addUri([uris], options)`
* `pin.pause(id)`
* `pin.unpause(id)`
* `pin.remove(id)`
* `pin.forceRemove(id)`
* `pin.removeDownloadResult(id)`
* `pin.pauseAll()` & `pin.unpauseAll()`

### Status Polling
* `pin.tellActive()`
* `pin.tellWaiting(offset, num)`
* `pin.tellStopped(offset, num)`
* `pin.tellStatus(id)`
* `pin.getGlobalStat()`
* `pin.changeGlobalOption(options)`
* `pin.getGlobalOption()`

---

## 4. Phase 1 Implementation Steps

1. **Initialization:** Run `cargo init` inside the `/Users/mac/Public/ArchitectureChange/pincer` directory.
2. **Dependencies:** Add `tokio`, `axum`, `serde`, `serde_json`, and `reqwest`.
3. **Draft the Types:** Create exact struct representations of `TaskStatus`, `GlobalStat`, and `RPCResponse` mapped to `.pincer` equivalents.
4. **WebSocket Loop:** Get the WebSockets echo server running and accepting connections from the Swift `Aria2RPCClient`.
5. **Basic Download:** Implement standard GET requests, mapping progress into the `.tellActive()` response list.
6. **Parallel Chunking:** Add range requests and `write_at` chunk integration.
7. **Pause/Resume:** Integrate `.pause()` channels dropping worker connections, and `.resume()` identifying completed intervals.
