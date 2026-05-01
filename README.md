Pincer - High-Performance Rust Download Engine
============================================

Disclaimer
----------
This program comes with no warranty.
You must use this program at your own risk.

Introduction
------------

Pincer is a modern, multithreaded download engine written in Rust. It is designed to be a lightweight, safe, and high-performance replacement for the Aria2 featureset, specifically tailored for modern desktop applications. 

Like aria2, Pincer is a utility for downloading files. It supports concurrent segmented downloading, maximizing bandwidth by dividing files into chunks and downloading them across multiple parallel connections. Pincer ensures sequential resume, efficiently continuing downloads from exactly where they left off by utilizing HTTP Range headers. 

Features
--------

Here is a list of features:

* Command-line interface
* Download files through HTTP(S)
* Concurrent Segmented downloading
* Sequential Resume utilizing HTTP Range headers
* Native Memory Safety through Rust
* Sparse Writing using `write_at` for direct disk I/O
* JSON-RPC (over WebSocket) interface for real-time status updates
* 1:1 Feature parity mapping with standard Aria2 RPC commands
* Multi-threaded Chunking and Worker Pool (Axel Pattern)
* Global throughput stats management

Versioning and release schedule
-------------------------------

We use standard semantic versioning (MAJOR.MINOR.PATCH) for Pincer releases. The MAJOR version will stay at 0 during the initial development and stabilization phase.

How to get source code
----------------------

We maintain the source code in the Pincer project repository.
To get the latest source code, navigate to the `pincer` directory:

    $ cd path/to/pincer

Dependency
----------

======================== ========================================
features                  dependency
======================== ========================================
Core Runtime             Rust (rustc, cargo), tokio
JSON-RPC Interface       axum
======================== ========================================

How to build
------------

Pincer is written in Rust. To build Pincer from the source package, you need the Rust toolchain installed.

1. Install Rust:
    $ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

2. Reload your environment:
    $ source $HOME/.cargo/env

To run the engine directly for testing and debugging:
    $ cargo run

To compile a highly optimized standalone binary for production:
    $ cargo build --release

After a release build, the executable is located at `target/release/pincer`.

Command-line usage
------------------

For quick direct download tests without using the RPC server:
    $ ./target/release/pincer "https://example.com/file.zip"

This will download the file to the current directory using multiple threads by default.

WebSocket / JSON-RPC
--------------------

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. 
The WebSocket server embedded in Pincer listens on port `6842` by default (`ws://127.0.0.1:6842/jsonrpc`).
It implements a 1:1 mapping with Aria2's JSON-RPC interface. 

Architecture
------------

Pincer is built on the `tokio` async runtime and consists of three primary layers:
1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool (Axel Pattern)**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to the disk using zero-allocation writes.

References
----------

* `JSON-RPC API <docs/JSON_RPC_API.md>`_
