# Roadmap

Dependency migration strategy and technical optimization paths for Pincer Engine.

> For what features are done and what's next, see [Feature Status](06-feature-status.md).

---

## Current Architecture Overview

Pincer is built on the `tokio` async runtime and consists of three primary layers:
1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to the disk using zero-allocation writes.

For a deep-dive into each module, see [Architecture](02-architecture.md).

---

## Networking & Transfer (The Engine)

The core of your tool. Higher levels offer safety and edge-case handling; lower levels offer raw speed and absolute control.

| Current: `reqwest` | Alternative: `ureq` / `isahc` | Low Level: `hyper` | Low-Low Level: `TcpStream` |
| :--- | :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** | **Level 1: Raw** |
| **Handling:** Handles redirects, cookies, TLS, and connection pooling automatically. | **Handling:** Lightweight. `ureq` is synchronous (good for threading); `isahc` is async but leaner than reqwest. | **Handling:** No high-level abstractions. You must manually build the Request object and handle the Body stream. | **Handling:** You write `GET / HTTP/1.1\\r\\n` to a socket. You must handle TLS handshakes (via `rustls`) yourself. |
| **Speed:** High, but overhead in binary size. | **Speed:** Slightly faster cold-start. | **Speed:** Peak performance; minimal overhead. | **Speed:** Theoretically fastest; practically dangerous without expert logic. |

## Server Interface (API/Remote Control)

| Current: `axum` | Alternative: `tiny_http` | Low Level: `std::net::TcpListener` |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Built on `tokio` and `tower`. Massive feature set for routing and middleware. | A tiny, synchronous, zero-dependency HTTP server library. | A simple loop that accepts connections. You must parse the HTTP headers manually. |

## Serialization (Data Handling)

| Current: `serde` / `serde_json` | Alternative: `miniserde` / `nanoserde` | Low Level: String Templates |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Heavy macro usage. Handles every edge case of JSON/YAML/Toml perfectly. | Strips out the complex features to provide faster compile times and fewer dependencies. | Use `format!` or `concat!` to manually build JSON strings for output. No parsing safety. |

## Command Line Interface (CLI)

| Current: `clap` | Alternative: `lexopt` | Low Level: `std::env::args()` |
| :--- | :--- | :--- |
| **Level 4: High** | **Level 3: Medium** | **Level 2: Systems** |
| Automatic `--help`, type validation, and shell completion. Pulls in many crates. | A minimalist, zero-dependency parser. You write the `while` loop, it gives you the tokens. | Manually iterate over the argument vector and use `match` statements to find flags. |

## Utilities & Logic

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
