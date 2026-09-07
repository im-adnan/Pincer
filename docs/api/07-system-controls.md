# System Controls

Methods for managing engine lifecycle, session control, and batch operations.

> **Previous:** [Events](06-events.md)

---

## Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.purgeDownloadResult` | Purges completed/error/removed downloads. | `[]` | `OK` (String) |
| `pin.removeDownloadResult` | Removes a specific task from memory. | `[gid (String)]` | `OK` (String) |
| `pin.getVersion` | Returns engine version information. | `[]` | `Version` (Object) |
| `pin.getSessionInfo` | Returns the current session ID. | `[]` | `SessionInfo` (Object) |
| `pin.shutdown` | Gracefully saves session and stops the engine. | `[]` | `OK` (String) |
| `pin.forceShutdown` | Immediately exits the engine without saving. | `[]` | `OK` (String) |
| `pin.saveSession` | Manually triggers a session save to disk. | `[]` | `OK` (String) |
| `system.multicall` | Executes multiple JSON-RPC calls in a single batch. | `[calls (Array of Objects)]` | Array of Results |
| `system.listMethods` | Returns all available JSON-RPC methods. | `[]` | Array of Strings |
| `system.listNotifications` | Returns all possible WebSocket notifications. | `[]` | Array of Strings |
