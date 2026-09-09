# Pincer Engine Documentation

Welcome to the Pincer Engine docs. Start from the top and work your way down — everything is numbered in reading order.

---

## Getting Started

1. [How It Works](01-how-it-works.md) — A jargon-free look at what Pincer does under the hood.
2. [User Guide](02-user-guide.md) — Start downloading files in minutes.
3. [CLI Reference](03-cli-reference.md) — Every flag and option, with examples.

---

## JSON-RPC API

How to control Pincer programmatically over WebSocket.

1. [Connecting](api/01-connecting.md) — WebSocket URL, port, and token authentication.
2. [Managing Downloads](api/02-managing-downloads.md) — Adding, pausing, removing, and queuing tasks.
3. [Checking Status](api/03-checking-status.md) — Querying progress, file lists, and global stats.
4. [Configuration](api/04-configuration.md) — Engine defaults, task options, and system keys.
5. [Advanced Features](api/05-advanced-features.md) — URL resolution, format conversion, Metalink, FTP/SFTP, and more.
6. [Events](api/06-events.md) — Real-time WebSocket notifications.
7. [System Controls](api/07-system-controls.md) — Version info, shutdown, session saves, and batch calls.

---

## Guides

Deep-dive walkthroughs for specific use cases.

1. [Session & Resume](guides/01-session-and-resume.md) — How auto-save and download resumption work.
2. [Swift Integration](guides/02-swift-integration.md) — Embedding Pincer in a macOS Swift/SwiftUI app.
3. [API Comparison](guides/03-api-comparison.md) — Pincer's methods mapped against standard download engines.

---

## Contributing

Everything you need to contribute code to Pincer.

1. [Getting Started](contributing/01-getting-started.md) — Setup, code style, and PR workflow.
2. [Architecture](contributing/02-architecture.md) — System design, modules, and concurrency model.
3. [Testing](contributing/03-testing.md) — Test suite architecture and how to run tests.
4. [Build & Release](contributing/04-build-and-release.md) — Compiling from source and cutting releases.
5. [Roadmap](contributing/05-roadmap.md) — Dependency migration strategy and optimization paths.
6. [Feature Status](contributing/06-feature-status.md) — What's done, what's next.

---

## License

[![GNU GPL v3](https://www.gnu.org/graphics/gplv3-127x51.png)](https://www.gnu.org/licenses/gpl-3.0.html)

Pincer Engine is free and open-source software licensed under the **[GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html)** (GPLv3).

