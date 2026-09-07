# Connection & Authentication

This document covers how to connect to Pincer's JSON-RPC interface and authenticate requests.

> **See also**: [API Overview](README.md) · [Task Management](TASK_MANAGEMENT.md) · [Events](EVENTS.md)

---

## Connection Details

- **Default Port**: `6842` (configurable via `-p` / `--port` CLI option)
- **WebSocket URL**: `ws://127.0.0.1:<PORT>/jsonrpc`
- **Protocol**: JSON-RPC 2.0

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. It implements a robust, proprietary JSON-RPC interface for full engine control.

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

> **Note**: All methods in the API reference use the `pin.*` namespace. The authentication token format is `"token:YOUR_SECRET"` and is always the first element in the `params` array when a secret is configured.
