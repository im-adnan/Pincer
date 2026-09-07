# Contributing to Pincer 🚀

First off, thank you for considering contributing to **Pincer**! Your help makes this high-performance download engine better for everyone.

This guide will help you set up your environment, understand the codebase, and submit your contributions.

> **Related Documentation**:
> * To understand the JSON-RPC interface, check the [**API Reference**](api/README.md).
> * For the CLI argument reference, see the [**CLI Reference**](guides/CLI_REFERENCE.md).
> * For the architectural breakdown, read the [**Architecture Specification**](ARCHITECTURE.md).
> * For testing instructions, read the [**Testing Guide**](TESTING.md).

---

## 💻 Getting Started

### Prerequisites
Pincer is written entirely in **Rust**. You will need the Rust toolchain installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build & Run
Please refer to our [Build and Release Guide](BUILD_AND_RELEASE.md) for detailed instructions on building, running, and creating releases.

---

## 📂 Code Architecture

Pincer strictly follows the **Single Responsibility Principle (SRP)**, isolating domain functions into modular packages of ~100 lines each inside `src/`:

*   **`src/cli/`** — Command-line interface parsing (`lexopt`), ANSI live progress renderers, and background daemonizer.
*   **`src/common/`** — Error types (`PincerError`), URI bracket expansion, and filename sanitization.
*   **`src/converter/`** — Media transcoding pipelines (`sips`, `cupsfilter`, `ffmpeg`, `afconvert`) with atomic rollback recovery.
*   **`src/engine/`** — Multi-threaded chunking, non-blocking zero-allocation disk I/O (`write_at`), rate limiting (`ThreadGuard`), and `.download` staging bundle creation.
*   **`src/manager/`** — Download orchestration facade, task scheduling, lifecycle state machine, speed calculations, and persistence (`pincer.session`).
*   **`src/metalink/`** — Metalink XML 3.0 and 4.0 parser.
*   **`src/models/`** — Strongly-typed domain models for tasks, sessions, RPC schemas, and torrent metadata.
*   **`src/protocol/`** — Pluggable protocol adapters for HTTP/HTTPS, FTP (`suppaftp`), and SFTP (`russh`/`russh-sftp`).
*   **`src/resolver/`** — Universal metadata discovery for URLs, Magnet links, `.torrent` payloads, and embedded HTML media.
*   **`src/rpc/`** — Axum WebSocket server, JSON-RPC 2.0 router, token authenticator, and granular method handlers.
*   **`src/torrent/`** — BitTorrent engine integration via `librqbit`, stats tracking, and selective file downloaders.

---

## 🧪 Testing

We provide a Python-based end-to-end integration test runner:

```bash
# Run the automated test suite
python3 scripts/run_tests.py
```

---

## 📜 Contribution Rules

Before submitting a pull request, please make sure you adhere to the following rules:

1. **Adhere to SRP (~100 lines per file)**: Keep modules focused on a single responsibility.
2. **Follow Rust naming conventions**: Use `snake_case` for variables/functions, `CamelCase` for types/structs. Run `cargo clippy` to catch common mistakes.
3. **Write descriptive commit messages**: Start with a short summary in the imperative mood (e.g., "Add proxy support").
4. **Update Documentation**: When modifying user-facing behavior or adding new features, update the relevant documentation in `README.md` or `docs/`.
5. **Zero regressions**: Ensure all tests compile cleanly and pass without warnings:
   ```bash
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   python3 -m unittest tests/test_rpc.py
   ```

---

## 📬 Pull Request Workflow

1.  **Fork** the repository and create your feature branch.
2.  Ensure your code builds cleanly and passes all lints.
3.  Write test cases where appropriate.
4.  Submit a **Pull Request (PR)** with a clear description of changes.

*Happy coding!*
