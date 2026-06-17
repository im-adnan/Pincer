Pincer - High-Performance Rust Download Engine
============================================

Disclaimer
----------
This program comes with no warranty.
You must use this program at your own risk.

Introduction
------------

Pincer is a modern, multithreaded download engine written entirely in Rust from the ground up. It is designed to be a lightweight, safe, and high-performance core tailored for modern desktop applications that require robust file transfer capabilities.

Pincer handles downloads by dividing files into dynamic segments and fetching them across multiple parallel connections to maximize bandwidth. It features an advanced persistence layer that ensures sequential resume, efficiently continuing downloads from exactly where they left off by utilizing HTTP Range headers.

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
* Standardized JSON-RPC interface for seamless integration
* Multi-threaded Chunking and Worker Pool
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

Please refer to the [Build and Release Guide](docs/BUILD_AND_RELEASE.md) for detailed instructions on setting up your environment, building the project, and managing releases.

Command-line usage
------------------

For quick direct download tests without using the RPC server:
    $ ./target/release/pincer "https://example.com/file.zip"

This will download the file to the current directory using multiple threads by default.

WebSocket / JSON-RPC
--------------------

Pincer features an integrated RPC layer powered by Axum, which handles incoming WebSocket connections and processes JSON-RPC commands. 
The WebSocket server embedded in Pincer listens on port `6842` by default (`ws://127.0.0.1:6842/jsonrpc`).
It implements a robust, proprietary JSON-RPC interface for full engine control.

Contributing
------------

We welcome contributions to Pincer! Please check out our [Contributing Guidelines](CONTRIBUTING.md) to learn how to set up your environment, follow our coding rules, and submit a pull request.

Architecture
------------

Pincer is built on the `tokio` async runtime and consists of three primary layers:
1. **RPC Layer (Axum)**: Handles incoming WebSocket connections and processes JSON-RPC commands.
2. **Management Layer**: Tracks download states, provides thread-safe access to task metadata, and manages global throughput stats.
3. **Worker Pool**: Each task spawns multiple workers that independently fetch segments and perform non-blocking concurrent writes to the disk using zero-allocation writes.

Documentation
-------------

Pincer is fully documented. Please refer to the following guides based on your needs:

*   **[JSON-RPC API & Comprehensive Manual](docs/USAGE.md)**: The complete user and developer manual. Contains all CLI arguments, configurations, and WebSocket RPC commands.
*   **[Build and Release Guide](docs/BUILD_AND_RELEASE.md)**: Detailed instructions on setting up your environment, building the project, and managing releases.
*   **[Contributing Guidelines](docs/CONTRIBUTING.md)**: The onboarding guide for new developers, including rules for pull requests and running tests.
*   **[Testing Guide](docs/TESTING.md)**: Complete details on running automated test runner, manual CLI/RPC tests, and writing new test cases.
*   **[Architecture & Future Roadmap](docs/FUTURE.md)**: An internal design document detailing the engine's technical direction, dependency targets, and strategies for maintaining peak speed.

## Support

If you find it valuable and would like to show your support by Starring this repository and introducing it to your friends, it would be greatly appreciated.
