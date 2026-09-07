# Checking Status

Methods for querying download progress, task metadata, and global engine statistics.

> **Previous:** [Managing Downloads](02-managing-downloads.md) · **Next:** [Configuration](04-configuration.md)

---

## Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.tellStatus` | Returns progress and status metadata. | `[gid (String), keys (Array of Strings, Optional)]` | `TaskStatus` (Object) |
| `pin.tellActive` | Returns a list of all currently active downloads. | `[keys (Array of Strings, Optional)]` | Array of `TaskStatus` |
| `pin.tellWaiting` | Returns a list of waiting/paused downloads. | `[offset (Int), num (Int), keys (Array, Optional)]` | Array of `TaskStatus` |
| `pin.tellStopped` | Returns a list of stopped downloads. | `[offset (Int), num (Int), keys (Array, Optional)]` | Array of `TaskStatus` |
| `pin.getGlobalStat` | Returns global statistics of the engine. | `[]` | `GlobalStat` (Object) |
| `pin.getUris` | Returns all source mirror URLs for a task. | `[gid (String)]` | Array of `PincerUri` |
| `pin.getFiles` | Returns the file list and selected states. | `[gid (String)]` | Array of `PincerFile` |
| `pin.getPeers` | Returns the active peers for a task. | `[gid (String)]` | Array (Stubbed) |
| `pin.getServers` | Returns connection speed stats per server. | `[gid (String)]` | Array of `PincerServer` |
