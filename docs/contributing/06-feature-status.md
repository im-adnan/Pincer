# Feature Status

Current implementation status of Pincer Engine features. This document tracks what has been achieved, what remains, and the prioritized roadmap for upcoming work.

> For dependency migration strategy, see [Roadmap](05-roadmap.md).

---

## Achieved Features

*   **Command-line interface** (lexopt)
*   **Download files through HTTP(S)**
*   **FTP / SFTP Protocol Support**: Seamless integration for anonymous and authenticated secure file transfers.
*   **Metalink Support**: Full support for `addMetalink` & XML parsing, enabling robust multi-source failover and SHA-256 checksum verification.
*   **Daemon Mode**: Running `pincer-engine` as a detached background service without a terminal window (`--daemon`).
*   **Concurrent Segmented downloading** (up to 99 threads)
*   **Sequential Resume utilizing HTTP Range headers**
*   **JSON-RPC (over WebSocket) interface** for real-time status updates
*   **Session Persistence & Resumption** (`pincer.session` auto-saving)
*   **Configuration File & Dynamic Options** (`pincer.conf` support, `changeOption`)
*   **Download / Upload Speed Throttling** (`max-download-limit`, `speed-mode`)
*   **Basic Advanced HTTP** (Proxy and Auth properties integrated into settings)

---

## Features Left to Implement

*   **Full RPC Standard Methods**: `system.multicall`, `system.listMethods`, `system.listNotifications`, `getSessionInfo`.
*   **Detailed Task Introspection RPCs**: `getFiles`, `getUris`, `getPeers`, `getServers`.
*   **Advanced Task Modification**: `changeUri`, `changePosition`, `forcePause`, `forcePauseAll`.
*   **Batch Downloading** (Parameterized URIs, Reading URIs from a text file)
*   **Netrc Support**
*   **BitTorrent Support**

---

## Prioritized Next Steps

1.  **Missing RPC Standard & Introspection Methods** (`getFiles`, `getUris`, `system.multicall`): Critical for GUI/Web UI frontends to display file contents.
2.  **Advanced Task Modification** (`changeUri`, `changePosition`): Highly requested for dynamic download environments.
3.  **Batch Downloading & Parameterized URIs**: Useful for downloading sequences (e.g., `image_{1..100}.jpg`).
