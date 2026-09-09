# Pincer — High-Performance Rust Download Engine

[![Rust](https://img.shields.io/badge/rust-stable-brightgreen.svg)](https://www.rust-lang.org/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0.html)
[![User Guide](https://img.shields.io/badge/Docs-User%20Guide-orange.svg)](docs/USER_GUIDE.md)

## Introduction

**Pincer** is a modern, ultra-high-performance download engine designed to get files to your computer as fast as your internet connection allows. 

Whether you're downloading a standard web file, a massive BitTorrent package, or an entire server directory via SFTP, Pincer automatically splits the file into pieces and downloads them all at the exact same time using multiple concurrent connections. Built completely in memory-safe Rust, it leverages zero-allocation disk writing to ensure your computer never slows down, even at peak gigabit speeds.

> **🌟 Love what you see?**
> If you find this project interesting, useful, or just want to support open-source Rust development, **please consider leaving a star!** It helps the project grow and reach more amazing contributors like you!

> **New to Pincer?** 
> - 📖 Read [How Pincer Works (For Everyone)](docs/HOW_IT_WORKS.md) for a simple explanation of concurrent downloads and BitTorrent.
> - 🚀 Read the [User Guide](docs/USER_GUIDE.md) to learn how to download files using the Command Line!

---

## 🤝 Calling All Contributors!

We are actively building the ultimate open-source download engine, and **we want your help!** Whether you're a Rust veteran or a newcomer looking for a friendly project to dive into, there is a place for you here.

- **First-time contributor?** Our codebase is heavily modularized with strict SRP (Single Responsibility Principle) rules, meaning you'll never be overwhelmed by monolithic files.
- **Ready to jump in?** Check out our [Contributing Guidelines](docs/developers/CONTRIBUTING.md) to get started!

---

## Key Features

- **Blazing Fast Concurrent Downloads**: Splits HTTP, HTTPS, FTP, and SFTP files into multiple streams.
- **Native BitTorrent Integration**: High-throughput torrent and magnet streaming powered by `librqbit`.
- **Zero-Allocation Disk I/O**: Writes data directly into pre-allocated spaces on your hard drive to prevent stuttering.
- **Robust Sequential Resume**: Automatically resumes interrupted transfers right where they left off.
- **macOS Staging Bundles**: Wraps active downloads in native macOS `.download` bundles with Gatekeeper management.
- **Built-in Media Transcoding**: Automatically converts images, PDFs, audio, and video using native tools.
- **Real-Time JSON-RPC 2.0**: A powerful WebSocket API for building Graphical User Interfaces (GUIs) on top of the engine.

---

## Quick Start

### Installation & Build

```bash
# Clone the repository
git clone https://github.com/Pincer-Engine/pincer-engine.git
cd pincer-engine

# Build in release mode
cargo build --release
```

### Direct CLI Download Mode

Download any URL directly with live progress tracking:

```bash
./target/release/pincer "https://example.com/largefile.iso" --split 8 --dir ~/Downloads
```

---

## Documentation Directory

We have organized our documentation to cater to both standard users and technical developers.

### For Users
- [How Pincer Works (Layman's Guide)](docs/HOW_IT_WORKS.md)
- [User Guide (Command Line Instructions)](docs/USER_GUIDE.md)

### For Developers
If you are building an app on top of Pincer, or want to contribute to the engine's Rust codebase, see our technical specs:
- [JSON-RPC 2.0 API Reference](docs/developers/API_REFERENCE.md)
- [Architecture & Design Specification](docs/developers/ARCHITECTURE.md)
- [Testing & Quality Assurance Guide](docs/developers/TESTING.md)
- [Build and Release Guide](docs/developers/BUILD_AND_RELEASE.md)
- [Contributing Guidelines](docs/developers/CONTRIBUTING.md)
- [Future Engine Roadmap](docs/developers/ROADMAP.md)

---

## License

[![GNU GPL v3](https://www.gnu.org/graphics/gplv3-127x51.png)](https://www.gnu.org/licenses/gpl-3.0.html)

Pincer Engine is free software: you can redistribute it and/or modify it under the terms of the **[GNU General Public License v3.0](https://www.gnu.org/licenses/gpl-3.0.html)** as published by the Free Software Foundation.

See the [LICENSE](LICENSE) file for full details.
