# Pincer: High-Performance Rust Download Engine

Pincer is a modern, multithreaded download engine written in Rust. It is designed to be a lightweight, safe, and high-performance replacement for the Aria2 featureset, specifically tailored for modern desktop applications.

## Key Features

- **Concurrent Segmented Downloading**: Maximizes bandwidth by dividing files into chunks and downloading them across multiple parallel connections.
- **Sequential Resume**: Efficiently continues downloads from exactly where they left off by utilizing HTTP Range headers.
- **Sparse Writing**: Writes directly to specific file offsets using `write_at`, eliminating the need for temporary files or concatenation.
- **Native Memory Safety**: Built in Rust to ensure memory safety and high concurrency without the overhead of older C-based engines.
- **WebSocket JSON-RPC**: A clean, modern interface for real-time status updates and task management.

## Getting Started

### Prerequisites

You will need the Rust toolchain installed to build Pincer.

1. Install Rust:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
2. Reload your environment:
   ```bash
   source $HOME/.cargo/env
   ```

### Building Pincer

1. **For Development**: Run the engine directly for testing and debugging.
   ```bash
   cargo run
   ```
2. **For Production**: Compile a highly optimized standalone binary.
   ```bash
   cargo build --release
   ```
   The resulting binary will be at `target/release/pincer`.

## Architecture

Pincer is built on the `tokio` async runtime and consists of three primary layers:

1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool (Axel Pattern)**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to the disk.

## Documentation Links

- **[API Reference](API_REFERENCE.md)**: Detailed JSON-RPC method documentation, providing 1:1 feature parity with standard Aria2 commands.
- **[Sluice Integration](SLUICE_INTEGRATION.md)**: Specific instructions for integrating Pincer into the Sluice SwiftUI application.
