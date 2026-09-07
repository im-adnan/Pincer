# Pincer JSON-RPC API Reference

Pincer exposes a JSON-RPC 2.0 interface over WebSocket for full engine control. All methods use the `pin.*` namespace.

---

## Quick Start

1. **Connect** to the WebSocket endpoint (see [Connection & Auth](CONNECTION.md))
2. **Send** a JSON-RPC request:
   ```json
   {
     "jsonrpc": "2.0",
     "method": "pin.addUri",
     "id": "1",
     "params": [
       ["https://example.com/file.zip"],
       {"dir": "/downloads", "split": 8}
     ]
   }
   ```
3. **Receive** real-time progress via [WebSocket Events](EVENTS.md)

---

## API Reference by Domain

| Document | Methods Covered |
|:---------|:----------------|
| [Connection & Auth](CONNECTION.md) | WebSocket URL, port config, token authentication |
| [Task Management](TASK_MANAGEMENT.md) | `pin.addUri`, `pin.addTorrent`, `pin.addMetalink`, `pin.remove`, `pin.pause`, `pin.unpause`, etc. |
| [Status & Monitoring](STATUS_AND_MONITORING.md) | `pin.tellStatus`, `pin.tellActive`, `pin.tellWaiting`, `pin.tellStopped`, `pin.getGlobalStat`, etc. |
| [Configuration](CONFIGURATION.md) | `pin.changeOption`, `pin.getOption`, `pin.changeGlobalOption`, defaults, system keys |
| [Advanced Features](ADVANCED_FEATURES.md) | URL resolution, format conversion, FTP/SFTP, Metalink, speed modes, trash, FIFO scheduling |
| [Events](EVENTS.md) | `pin.onDownloadStart`, `pin.onDownloadComplete`, `pin.onDownloadError`, etc. |
| [System](SYSTEM.md) | `pin.getVersion`, `pin.shutdown`, `pin.saveSession`, `system.multicall`, etc. |

---

## Related Guides

- [CLI Reference](../guides/CLI_REFERENCE.md) — Using Pincer from the command line
- [Session Persistence](../guides/SESSION_PERSISTENCE.md) — Auto-save, resumption, and session files
- [Swift Integration](../guides/SWIFT_INTEGRATION.md) — Embedding Pincer in a macOS app
- [API Comparison](../guides/API_COMPARISON.md) — Method mapping vs standard download engines

---

## Pincer Exclusive Advantages

*   **Native Memory Safety**: Written entirely in Rust, preventing the segfaults and memory leaks possible in other C-based engines.
*   **Async I/O Worker Pool**: Uses a highly concurrent `tokio` pattern for zero-allocation disk writes, offering lower CPU overhead on high-speed connections.
*   **First-Class WebSocket Layer**: Powered by `axum` for modern, efficient communication.
