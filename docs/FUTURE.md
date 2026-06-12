# Architecture Migration & Dependency Roadmap

> **Related Documentation**:
> * If you want to contribute to the project, please see our [**Contributing Guide**](CONTRIBUTING.md).
> * For the full user manual and WebSocket specifications, see the [**JSON-RPC API & Manual**](USAGE.md).

## Current Architecture Overview

Pincer is built on the `tokio` async runtime and consists of three primary layers:
1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to the disk using zero-allocation writes.

### Technical Architecture (Rust Backend)
*   **Native Memory Safety**: Written entirely in Rust, preventing the segfaults and memory leaks common in C++ codebases[cite: 1, 2].
*   **Async I/O Worker Pool**: Built on `tokio` for high-throughput, low-latency processing via a concurrent worker pool[cite: 2, 3].
*   **Zero-Allocation Disk Writes**: On Unix-based systems, Pincer uses `write_at` to write data chunks directly to their final positions on disk, minimizing CPU overhead[cite: 3].
*   **Range Support Detection**: Automatically detects if a server supports `Accept-Ranges`; if not supported, it gracefully falls back to single-threaded mode[cite: 3].

## 1. Networking & Transfer (The Engine)
The core of your tool. Higher levels offer safety and edge-case handling; lower levels offer raw speed and absolute control.

| Current: `reqwest` | Alternative: `ureq` / `isahc` | Low Level: `hyper` | Low-Low Level: `TcpStream` |
| :--- | :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** | **Level 1: Raw** |
| **Handling:** Handles redirects, cookies, TLS, and connection pooling automatically. | **Handling:** Lightweight. `ureq` is synchronous (good for threading); `isahc` is async but leaner than reqwest. | **Handling:** No high-level abstractions. You must manually build the Request object and handle the Body stream. | **Handling:** You write `GET / HTTP/1.1\r\n` to a socket. You must handle TLS handshakes (via `rustls`) yourself. |
| **Speed:** High, but overhead in binary size. | **Speed:** Slightly faster cold-start. | **Speed:** Peak performance; minimal overhead. | **Speed:** Theoretically fastest; practically dangerous without expert logic. |

## 2. Server Interface (API/Remote Control)
If you need to control your downloader via a browser or another app.

| Current: `axum` | Alternative: `tiny_http` | Low Level: `std::net::TcpListener` |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Built on `tokio` and `tower`. Massive feature set for routing and middleware. | A tiny, synchronous, zero-dependency HTTP server library. | A simple loop that accepts connections. You must parse the HTTP headers manually. |

## 3. Serialization (Data Handling)
Necessary for configuration files and JSON-RPC interfaces.

| Current: `serde` / `serde_json` | Alternative: `miniserde` / `nanoserde` | Low Level: String Templates |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Heavy macro usage. Handles every edge case of JSON/YAML/Toml perfectly. | Strips out the complex features to provide faster compile times and fewer dependencies. | Use `format!` or `concat!` to manually build JSON strings for output. No parsing safety. |

## 4. Command Line Interface (CLI)
How the user interacts with the tool.

| Current: `clap` | Alternative: `lexopt` | Low Level: `std::env::args()` |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Automatic `--help`, type validation, and shell completion. Pulls in many crates. | A minimalist, zero-dependency parser. You write the `while` loop, it gives you the tokens. | Manually iterate over the argument vector and use `match` statements to find flags. |

## 5. Utilities & Logic

| Component | Level 4: Heavy | Level 3: Lean | Level 2: Standard/Zero-Dep |
| :--- | :--- | :--- | :--- |
| **Regex** | `regex` (Full engine) | `glob` (For file patterns) | `str::find` & `str::split_once` |
| **UUID** | `uuid` (Standard compliant) | `rand` (Just random bits) | `SystemTime` + counter |
| **Trash** | `trash` (Cross-platform bin) | N/A | `std::fs::remove_file` (Permanent) |
| **Encoding** | `percent-encoding` | N/A | Manual `replace("%20", " ")` logic |
| **Logging** | `log` + `env_logger` | `simple_logger` | `eprintln!` macro |

---

## Technical Strategy for Faster Download Speeds

As you move from **High-Level** to **Low-Level**, your ability to optimize speed increases, but your "Edge-Case" safety decreases.

### To Maintain Max Speed with Min Dependencies:
1.  **Direct I/O (The "Pincer" Secret):**
    Regardless of the library, use `file.set_len()` to pre-allocate space on disk. This prevents filesystem fragmentation during multi-connection downloads.
    

2.  **Zero-Copy Buffering:**
    Instead of using `bytes` crate (Level 4), use `std::io::BufWriter` with a manually tuned buffer size (usually 64KB or 128KB) to match the CPU cache line.

3.  **The Resumption Logic:**
    To handle edge cases (flaky servers) without `reqwest`:
    *   **Level 4:** Let the library handle retries.
    *   **Level 2:** Implement a "Retry Loop" around your `TcpStream`. Check the `Content-Length` vs the local file size. Send `Range: bytes=N-` where $N$ is your current progress.
    

---

## Summary Checklist for Migration

*   **Priority 1: Speed?** Stick with `tokio` for the executor, but move from `reqwest` to `hyper`. This keeps the async efficiency but removes high-level bloat.
*   **Priority 2: Zero Dependencies?** Move everything to the **Level 1/2** column. You will gain a <1MB binary, but you must manually code the "Handshake" and "Retry" logic for every protocol.
*   **Priority 3: Edge Case Handling?** Stay at **Level 4**. The open-source community has already fixed the bugs you haven't encountered yet (e.g., specific header formats for old Nginx servers).

---

## Aria2 Feature Comparison & Feature Roadmap

As part of our goal to build a modern replacement for `aria2`, here is a breakdown of what we have achieved and what remains.

### Recreated Features (Achieved)
*   **Command-line interface** (lexopt)
*   **Download files through HTTP(S)**
*   **Concurrent Segmented downloading** (up to 99 threads)
*   **Sequential Resume utilizing HTTP Range headers**
*   **JSON-RPC (over WebSocket) interface** for real-time status updates
*   **Session Persistence & Resumption** (`pincer.session` auto-saving)
*   **Configuration File & Dynamic Options** (`pincer.conf` support, `changeOption`)
*   **Download / Upload Speed Throttling** (`max-download-limit`, `speed-mode`)
*   **Basic Advanced HTTP** (Proxy and Auth properties integrated into settings)

### Features Left from Aria2 (Apart from BitTorrent)
*   **Full RPC Standard Methods**: `system.multicall`, `system.listMethods`, `system.listNotifications`, `getSessionInfo`.
*   **Detailed Task Introspection RPCs**: `getFiles`, `getUris`, `getPeers`, `getServers`.
*   **Advanced Task Modification**: `changeUri`, `changePosition`, `forcePause`, `forcePauseAll`.
*   **FTP / SFTP Protocol Support**
*   **Metalink Support** (`addMetalink` & XML parsing)
*   **Batch Downloading** (Parameterized URIs, Reading URIs from a text file)
*   **Netrc Support**
*   **Daemon Mode** (Running as a detached background service)

### Advanced Features to Add (Prioritized Roadmap)

1.  **Missing RPC Standard & Introspection Methods** (`getFiles`, `getUris`, `system.multicall`): Critical for GUI/Web UI frontends to display file contents and support standard Aria2 clients.
2.  **Advanced Task Modification** (`changeUri`, `changePosition`): Highly requested for dynamic download environments.
3.  **FTP / SFTP Protocol Support**: Expands the engine beyond HTTP/HTTPS.
4.  **Batch Downloading & Parameterized URIs**: Useful for downloading sequences (e.g., `image_{1..100}.jpg`).
5.  **Metalink Support**: For robust distributed downloading and chunk checksum validation.
6.  **Daemon Mode**: To allow `pincer-engine` to run cleanly as a background service without a terminal window.
