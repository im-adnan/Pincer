# Task Management Methods

Methods for adding, removing, pausing, and controlling download tasks.

> **See also**: [Connection & Auth](CONNECTION.md) · [Status & Monitoring](STATUS_AND_MONITORING.md) · [Events](EVENTS.md)

---

## Methods

| Method | Description | Parameters | Returns |
| :--- | :--- | :--- | :--- |
| `pin.addUri` | Adds a new download task from one or more URIs. | `[uris (Array of Strings), options (Object, Optional), position (Integer, Optional)]` | `gid` (String) |
| `pin.addMetalink` | Adds a download by providing a base64-encoded Metalink XML string. | `[metalink (Base64 String), options?, position?]` | `gid` (String) |
| `pin.addTorrent` | Adds a BitTorrent download by uploading a ".torrent" file. | `[torrent (Base64 String), uris (Array of Strings, Optional), options?, position?]` | `gid` (String) |
| `pin.remove` | Removes the download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.removeAndFile` | Removes the download and moves its file to Trash. | `[gid (String)]` | `true` (Boolean) |
| `pin.forceRemove` | Immediately removes the download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.pause` | Pauses the active/waiting download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.forcePause` | Forcefully pauses the download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.unpause` | Unpauses the paused download denoted by `gid`. | `[gid (String)]` | `gid` (String) |
| `pin.pauseAll` | Pauses all active/waiting downloads. | `[]` | `OK` (String) |
| `pin.forcePauseAll` | Forcefully pauses all active/waiting downloads. | `[]` | `OK` (String) |
| `pin.unpauseAll` | Unpauses all paused downloads. | `[]` | `OK` (String) |
| `pin.changePosition` | Adjusts the queue position of a download. | `[gid (String), pos (Int), how (String)]` | `0` (Int) |
| `pin.changeUri` | Dynamically removes and adds mirror URLs to a task. | `[gid (String), fileIndex (Int), delUris (Array), addUris (Array)]` | Array of Results |
| `pin.resolveUrl` | Resolves a URL to its final direct download link. | `[url (String)]` | `ResolveResponse` (Object) |

---

## Key Task Options

When passing an `options` object to `pin.addUri` or `pin.changeOption`, the following keys are used:

- `dir`: Target directory to store the file.
- `out`: The file name of the downloaded file.
- `split` or `-s`: (Integer) Number of connections to use (Default: 5).
- `max-connection-per-server` or `-x`: (Integer) Max connections to a single server (Default: 1).
- `min-split-size` or `-k`: (String) Minimum size to split a chunk (e.g., `1M`).
- `max-download-limit`: (String) Speed limit for the download (e.g., `50K`, `0` for unlimited).
- `header`: (Array of Strings) Custom HTTP Headers.
- `user-agent`: (String) Custom User-Agent string.

> **See also**: For the full list of configuration keys and defaults, see [Configuration](CONFIGURATION.md).
