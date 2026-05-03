# Architecture Migration & Dependency Roadmap

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

