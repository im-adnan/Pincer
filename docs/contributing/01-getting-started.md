# Getting Started as a Contributor

Thank you for considering contributing to **Pincer**! Your help makes this high-performance download engine better for everyone.

---

## Prerequisites

You'll need the Rust toolchain. See [Build & Release](04-build-and-release.md) for full setup instructions.

---

## Code Style & SRP Rules

Before submitting a pull request, please adhere to the following rules:

1. **Adhere to SRP (~100 lines per file)**: Keep modules focused on a single responsibility. See [Architecture](02-architecture.md) for how the codebase is organized.
2. **Follow Rust naming conventions**: Use `snake_case` for variables/functions, `CamelCase` for types/structs. Run `cargo clippy` to catch common mistakes.
3. **Write descriptive commit messages**: Start with a short summary in the imperative mood (e.g., "Add proxy support").
4. **Update Documentation**: When modifying user-facing behavior or adding new features, update the relevant documentation in `docs/`.
5. **Zero regressions**: Ensure all tests pass. See [Testing](03-testing.md) for how to run the test suite.

---

## Pull Request Workflow

1.  **Fork** the repository and create your feature branch.
2.  Ensure your code builds cleanly and passes all lints:
    ```bash
    cargo fmt --all -- --check
    cargo clippy --all-targets -- -D warnings
    ```
3.  Write test cases where appropriate.
4.  Submit a **Pull Request (PR)** with a clear description of changes.

*Happy coding!*
