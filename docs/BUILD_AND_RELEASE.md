# Build and Release Guide

This document covers everything you need to know about building Pincer from source, setting up the development environment, and managing new releases.

## Prerequisites

Pincer is written entirely in **Rust**. You will need the Rust toolchain installed:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

After installation, reload your environment:
```bash
source $HOME/.cargo/env
```

## How to Build

1. **Clone the repository:**
   ```bash
   git clone https://github.com/<GITHUB_OWNER>/Pincer-Engine.git
   cd Pincer-Engine
   ```

   > If you use environment variables in your scripts or CI, set `GITHUB_OWNER` to the GitHub account and `GITHUB_REPO` to `Pincer-Engine`.

   If you use local automation, make sure `GITHUB_OWNER` and `GITHUB_REPO` are defined before running scripts that reference repository URLs.

2. **Run in development/RPC server mode (defaults to port `6842` over WebSocket):**
   ```bash
   cargo run
   ```

3. **Run the standalone CLI to test direct downloads:**
   ```bash
   TEST_URL="https://images.unsplash.com/photo-1446941303752-a64bb1048d54?ixlib=rb-4.1.0&q=85&fm=jpg&crop=entropy&cs=srgb&dl=nasa-U2uKrI4lci8-unsplash.jpg"
   cargo run -- "$TEST_URL" -s 8 -d ./
   ```

4. **Compile a highly optimized standalone binary for production:**
   ```bash
   cargo build --release
   ```
   After a release build, the executable is located at `target/release/pincer`.

## How to Release

We use a unified release script to ensure that all checks pass before a release version is tagged. 

The `scripts/release.sh` script automates:
1. **CI validation:** Executes the unified test suite (`python3 tests/run_tests.py --ci`), which performs formatting checks, linting, Rust unit tests, and the full suite of Python integration tests.
2. **Release build:** Ensures the application compiles successfully under `--release`.
3. **Version bump:** Updates the version automatically inside `Cargo.toml`.
4. **Git Tagging:** Commits the version bump and tags the commit with the new version (e.g. `v1.5.0`).

### Running the Release Script

You must provide the new semantic version as an argument:

```bash
./scripts/release.sh <new_version>
```

**Example:**
```bash
./scripts/release.sh 1.5.0
```

Once the script completes successfully, you will be prompted to push the new tag and commit:

```bash
git push origin <branch-name> --tags
```

Pushing the `v*` tag triggers the `.github/workflows/release.yml` GitHub Action, which builds and publishes the pre-compiled `.zip` artifacts to the GitHub Releases page automatically.
