# Events

Pincer streams real-time status notifications to all connected WebSocket clients. You do not need to poll for updates.

> **Previous:** [Advanced Features](05-advanced-features.md) · **Next:** [System Controls](07-system-controls.md)

---

## Event Reference

| Event | Scenario |
| :--- | :--- |
| `pin.onDownloadStart` | Triggered when a task enters **active** state. |
| `pin.onDownloadPause` | Triggered when a task is manually paused. |
| `pin.onDownloadComplete` | Triggered when a task finishes successfully. |
| `pin.onDownloadError` | Triggered when a task fails. |
| `pin.onDownloadStop` | Triggered when a task is stopped. |
| `pin.onBtDownloadComplete` | Triggered when a BitTorrent download completes. |

---

## Notification Format

All events follow the standard JSON-RPC 2.0 notification format (no `id` field):

```json
{
  "jsonrpc": "2.0",
  "method": "pin.onDownloadComplete",
  "params": [{"gid": "2089b05ecca3d829"}]
}
```

The `params` array contains a single object with the `gid` (task ID) that triggered the event.
