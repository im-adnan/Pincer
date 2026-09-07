# Connecting to Pincer

How to connect to Pincer's JSON-RPC interface and authenticate your requests.

> **Next:** [Managing Downloads](02-managing-downloads.md)

---

## Connection Details

- **Default Port**: `6842` (configurable via `-p` / `--port` CLI option)
- **WebSocket URL**: `ws://127.0.0.1:<PORT>/jsonrpc`
- **Protocol**: JSON-RPC 2.0

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. All methods use the `pin.*` namespace.

---

## Authentication

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

## What Makes Pincer Different

*   **Native Memory Safety**: Written entirely in Rust, preventing the segfaults and memory leaks possible in other C-based engines.
*   **Async I/O Worker Pool**: Uses a highly concurrent `tokio` pattern for zero-allocation disk writes, offering lower CPU overhead on high-speed connections.
*   **First-Class WebSocket Layer**: Powered by `axum` for modern, efficient communication.
