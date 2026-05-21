# Contributing to Pincer 🚀

First off, thank you for considering contributing to **Pincer**! Your help is what makes this high-performance download engine better for everyone.

This guide will help you set up your environment, understand the codebase, and submit your contributions.

> **Related Documentation**:
> * To understand how the JSON-RPC interface and CLI arguments work, check the [**Comprehensive Manual & API Reference**](USAGE.md).
> * For insights into Pincer's long-term technical direction and dependency architecture, read our [**Architecture Migration Roadmap**](FUTURE.md).

---

## 💻 Getting Started

### Prerequisites
Pincer is written entirely in **Rust**. You will need the Rust toolchain installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build & Run
1. Clone the repository:
   ```bash
   git clone https://github.com/im-adnan/pincer.git
   cd pincer
   ```
2. Build the project:
   ```bash
   cargo build
   ```
3. Run the standalone CLI to test direct downloads:
   ```bash
   cargo run -- "https://example.com/dummy.zip" -s 8 -d ./
   ```
4. Run in RPC server mode (defaults to port `6842` over WebSocket):
   ```bash
   cargo run
   ```
5. Compile a highly optimized standalone binary for production:
   ```bash
   cargo build --release
   ```
   After a release build, the executable is located at `target/release/pincer`.

---

## 📂 Code Architecture

To help you navigate, here is how the core modules are structured inside `src/`:

*   **`main.rs`** — Entry point. Handles command-line arguments (using `lexopt`), routes direct CLI downloads, or fires up the background Axum RPC server.
*   **`models.rs`** — Defines JSON-RPC payloads, notifications, and serialization structs matching industry-standard protocol shapes.
*   **`worker.rs`** — The low-level segment worker. Handles range connections, processes speed limits, and performs zero-allocation disk I/O using Unix thread-safe `write_at`.
*   **`task.rs`** — Orchestrates the download task. Performs HEAD/GET metadata discovery, checks range acceptability, and divides the byte spaces into worker chunks.
*   **`manager.rs`** — The global task registry. Controls session loading/saving (`pincer.session`), global options, speed modes, and lifecycle states (active, waiting, stopped).
*   **`rpc.rs`** — The WebSocket networking layer. Listens on `/jsonrpc` using Axum and channels commands/events between the clients and the Manager.

---

## 🧪 Testing

We include utility scripts to quickly verify the server's JSON-RPC interface and verify all dynamic features.

1.  Start the Pincer RPC server:
    ```bash
    cargo run
    ```
2.  In another terminal, run the validation test:
    ```bash
    python3 verify_features.py
    ```
    This script will automatically connect via websockets, add a test download task, poll its status, pause, resume, and remove the task to ensure zero regression.

---

## 📜 Contribution Rules

Before submitting a pull request, please make sure you adhere to the following rules:

1. **Keep PRs small and focused**: A PR should ideally address a single issue or implement a single feature. Large PRs that mix refactoring with feature additions are difficult to review.
2. **Follow Rust naming conventions**: Use `snake_case` for variables/functions, `CamelCase` for types/structs. Run `cargo clippy` to catch common mistakes.
3. **Write descriptive commit messages**: Start with a short summary in the imperative mood (e.g., "Add proxy support"). Provide additional details in the commit body if necessary.
4. **Update Documentation**: If your change modifies user-facing behavior or adds new features, please update the relevant documentation in `README.md` or the `docs/` folder.
5. **No regressions**: Make sure all existing tests pass and consider adding new tests to verify your changes. Run `python3 verify_features.py` before submitting.

---

## 📬 Pull Request Workflow

1.  **Fork** the repository and create your branch from `main`.
2.  Ensure your code builds cleanly and is formatted using standard rust lints:
    ```bash
    cargo fmt --all
    cargo clippy --all-targets
    ```
3.  Write tests or scripts where appropriate to verify your feature.
4.  Submit a **Pull Request (PR)** with a descriptive title and description of your modifications.
5.  Our automated GitHub Actions workflow will build and lint your changes to ensure compilation integrity.

---

## 🗺️ Contribution Wishlist / Help Wanted

Are you looking for some inspiration on what to contribute? Check out our roadmap:
*   **BitTorrent Protocol Engine** (Implementation of peer storage, piece selection, and DHT in Rust).
*   **FTP & SFTP Protocols**.
*   **Proxy Integration** (HTTP/HTTPS/SOCKS5 support).
*   **Cookie Storage & Parsing** (Netscape/Sqlite engines).
*   **Additional RPC methods** (e.g., `system.multicall`, `pin.getFiles`).

*Happy coding!*
