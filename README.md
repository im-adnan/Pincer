# Pincer — High-Performance Rust Download Engine

## Disclaimer

This program comes with no warranty. You must use this program at your own risk.

## Introduction

Pincer is a modern, multithreaded download engine written entirely in Rust from the ground up. It is designed to be a lightweight, safe, and high-performance core tailored for modern desktop applications that require robust file transfer capabilities.

Pincer handles downloads by dividing files into dynamic segments and fetching them across multiple parallel connections to maximize bandwidth. It features an advanced persistence layer that ensures sequential resume, efficiently continuing downloads from exactly where they left off by utilizing HTTP Range headers.

## Features

- Command-line interface
- Download files through HTTP(S)
- Concurrent segmented downloading
- Sequential resume utilizing HTTP Range headers
- Native memory safety through Rust
- Sparse writing using `write_at` for direct disk I/O
- JSON-RPC (over WebSocket) interface for real-time status updates
- Multi-threaded chunking and worker pool
- Global throughput stats management

## Versioning and Release Schedule

We use standard semantic versioning (`MAJOR.MINOR.PATCH`) for Pincer releases. Releases are automated via GitHub Actions and triggered by pushing a git tag.

## Getting the Source Code

Clone the repository from GitHub:

```bash
git clone https://github.com/im-adnan/Pincer-Engine
cd Pincer-Engine
```

## Dependencies

| Feature | Dependency |
|---|---|
| Core Runtime | Rust (`rustc`, `cargo`), `tokio` |
| JSON-RPC Interface | `axum` |
| HTTP Downloads | `reqwest` |
| FTP/SFTP Support | `suppaftp`, `russh`, `russh-sftp` |

## How to Build

Please refer to the [Build and Release Guide](docs/BUILD_AND_RELEASE.md) for detailed instructions on setting up your environment, building the project, and managing releases.

## Command-Line Usage

For quick direct download tests without using the RPC server:

```bash
./target/release/pincer "https://example.com/file.zip"
```

This will download the file to the current directory using multiple threads by default.

## WebSocket / JSON-RPC

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. The WebSocket server listens on port `6842` by default (`ws://127.0.0.1:6842/jsonrpc`) and implements a robust JSON-RPC interface for full engine control.

## Contributing

We welcome contributions to Pincer! Please check out our [Contributing Guidelines](docs/CONTRIBUTING.md) to learn how to set up your environment, follow our coding rules, and submit a pull request.

## Architecture

Pincer is built on the `tokio` async runtime and consists of three primary layers:

1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to disk using zero-allocation writes.

## Documentation

Pincer is fully documented. Refer to the following guides based on your needs:

- **[JSON-RPC API & Comprehensive Manual](docs/USAGE.md)**: The complete user and developer manual. Contains all CLI arguments, configurations, and WebSocket RPC commands.
- **[Build and Release Guide](docs/BUILD_AND_RELEASE.md)**: Detailed instructions on setting up your environment, building the project, and managing releases.
- **[Contributing Guidelines](docs/CONTRIBUTING.md)**: The onboarding guide for new developers, including rules for pull requests and running tests.
- **[Testing Guide](docs/TESTING.md)**: Complete details on running the automated test runner, manual CLI/RPC tests, and writing new test cases.
- **[Architecture & Future Roadmap](docs/FUTURE.md)**: An internal design document detailing the engine's technical direction and strategies for maintaining peak speed.

## Support

If you find Pincer valuable, please consider starring the repository and sharing it with others — it is greatly appreciated.
