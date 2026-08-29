# Pincer Architecture & Design Specification

This document provides a comprehensive technical overview of the Pincer architecture, internal subsystems, concurrency models, and domain modules.

---

## 1. Architectural Philosophy & SRP Principles

Pincer is architected strictly around the **Single Responsibility Principle (SRP)**. Every source file is isolated to a focused domain responsibility (~100 lines per module) to ensure maintainability, auditability, and safety.

```
pincer-engine/
├── src/
│   ├── cli/            # Command-line interface, argument parsing, ANSI UI, daemonizer
│   ├── common/         # Common utilities, error types, URI expansion, sanitization
│   ├── converter/      # Media transcoding subsystem (sips, cupsfilter, ffmpeg, afconvert)
│   ├── engine/         # Core HTTP/FTP chunking, worker streams, disk allocation, throttling
│   ├── manager/        # Task registry, scheduling, persistence, lifecycle, query facade
│   ├── metalink/       # Metalink XML 3.0 / 4.0 file and hash parser
│   ├── models/         # Strongly-typed serde data transfer and domain models
│   ├── protocol/       # Protocol adapters (HTTP/S, FTP, SFTP)
│   ├── resolver/       # URL, Magnet, Torrent, and media scraping resolvers
│   ├── rpc/            # Axum WebSocket server, JSON-RPC 2.0 router, token auth, method handlers
│   └── torrent/        # BitTorrent integration engine (librqbit session, stats, file selector)
```

---

## 2. Core Subsystems

### 2.1 Protocol Adapter Layer (`src/protocol/`)
Provides uniform stream abstractions via the `ProtocolAdapter` trait:
- `HttpAdapter`: Asynchronous byte-range streaming and metadata discovery via `reqwest`.
- `FtpAdapter`: FTP/FTPS segmented streaming using `suppaftp`.
- `SftpAdapter`: SSH/SFTP streaming and metadata discovery using `russh` and `russh-sftp`.

### 2.2 Core Execution Engine (`src/engine/`)
- `DiskAllocator`: Zero-fragmentation space pre-allocation (`set_len`).
- `DownloadBundle`: macOS `.download` staging bundle structure, `Info.plist` UTI assignment, and `com.apple.quarantine` handling.
- `RangeChunker`: Parallel byte-range partitioner and resume offset arithmetic.
- `DownloadWorker`: Non-blocking zero-allocation disk writes using POSIX `write_at` on an `Arc<File>` across concurrent threads without lock contention.
- `RateThrottler`: Proportional bandwidth allocation using the `ThreadGuard` RAII active thread counter.

### 2.3 BitTorrent Integration Engine (`src/torrent/`)
- `TorrentSessionManager`: Configures `librqbit::Session` with port range `6881..6891` and automatic ephemeral port fallback (`0..1`).
- `TorrentTaskSpawner`: Inspects Magnet and Torrent byte sources and prepares isolated directory trees.
- `TorrentStatsTracker`: Computes instantaneous download/upload speeds, seeders, and peer snapshots.
- `TorrentFileSelector`: Cleans up unselected files and prunes empty directory branches.

### 2.4 Universal Media & Metadata Resolver (`src/resolver/`)
- `TorrentResolver`: List-only metadata extraction for Magnet links and Base64-encoded `.torrent` files.
- `DirectHttpResolver`: Content-Type, Content-Length, Content-Disposition, and Accept-Ranges resolution.
- `HtmlScraper` & `ScriptExtractor`: OpenGraph/Twitter card extraction, Next.js embedded JSON scraping, and direct video stream detection.

### 2.5 Media Transcoder (`src/converter/`)
- `ImageSips`: macOS `sips` image conversion (`jpeg`, `png`, `webp`, `heic`).
- `CupsPdf`: macOS `cupsfilter` raster-to-PDF conversion.
- `Ffmpeg`: Universal media transcoding.
- `Afconvert`: macOS `afconvert` audio encoding.
- Includes rollback restoration that re-establishes original files if conversion fails.

### 2.6 Central Manager (`src/manager/`)
- `TaskSpawner` & `TaskRunner`: Coordinates task lifecycle, execution, SHA256 integrity verification, and quarantine removal.
- `TaskLifecycleManager`: Safe pause/unpause state machine transitions.
- `TaskRemovalManager`: Atomic task deletion and non-blocking background Trash operations (`trash::delete`).
- `SessionPersistence`: Session serialization to `~/.pincer/pincer.session` and `.download/state.json`.

### 2.7 JSON-RPC 2.0 Layer (`src/rpc/`)
- Axum WebSocket server at `ws://127.0.0.1:6842/jsonrpc`.
- Native `pin.*` and `system.*` namespace dispatch.
- Per-connection authentication with `token:<secret>` support.
- Real-time broadcast notification channel (`pin.onDownloadStart`, `pin.onDownloadProgress`, `pin.onDownloadComplete`, etc.).

---

## 3. Concurrency & Disk I/O Model

```
                    ┌─────────────────────────┐
                    │    DownloadManager      │
                    └────────────┬────────────┘
                                 │
           ┌─────────────────────┼─────────────────────┐
           ▼                     ▼                     ▼
   ┌───────────────┐     ┌───────────────┐     ┌───────────────┐
   │ Worker 1      │     │ Worker 2      │     │ Worker N      │
   │ (Range 0..A)  │     │ (Range A..B)  │     │ (Range B..End)│
   └───────┬───────┘     └───────┬───────┘     └───────┬───────┘
           │                     │                     │
           └─────────────────────┼─────────────────────┘
                                 ▼
                     ┌───────────────────────┐
                     │ Shared Arc<File>      │
                     │ (write_at POSIX I/O)  │
                     └───────────────────────┘
                                 │
                                 ▼
                     ┌───────────────────────┐
                     │ Staging .download     │
                     │ Bundle (with UTI)     │
                     └───────────────────────┘
```
