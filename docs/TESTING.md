# Testing Pincer

This document outlines the testing architecture and procedures for the Pincer RPC server. We use a Python-based test suite to verify the end-to-end functionality of the Rust backend.

## Why Python for Testing?
Using Python (specifically `unittest.IsolatedAsyncioTestCase` and `websockets`) allows us to perform black-box integration testing. By interacting with Pincer over WebSockets exactly as a real client would, we ensure that the compiled Rust binary and its RPC interface function correctly in real-world scenarios. It also allows us to quickly validate JSON-RPC structures without the boilerplate of a compiled test harness.

## Test Suite Architecture
The test suite is located in the `tests/` directory and is split into two parts:
1. `tests/test_rpc.py`: Tests the WebSocket JSON-RPC server daemon.
2. `tests/test_cli.py`: Tests the direct Command Line Interface (CLI) downloads.

It utilizes Python's built-in `unittest` module to minimize external dependencies.

### What is Tested?
The test suite covers the active features exposed by the `src/rpc.rs` backend, grouped into logical blocks:
1. **System & Info** (`test_01_get_version`, `test_06_misc`): 
   - `pin.getVersion`, `pin.resolveUrl`, `pin.saveSession`
2. **Global Options** (`test_02_global_options`): 
   - `pin.getGlobalOption`, `pin.changeGlobalOption`
3. **Task Lifecycle** (`test_03_lifecycle`): 
   - `pin.addUri`, `pin.tellStatus`, `pin.pause`, `pin.unpause`, `pin.changeOption`, `pin.getOption`, `pin.remove`, `pin.removeAndFile`, `pin.forceRemove`
4. **Bulk Operations & Stats** (`test_04_bulk_and_stats`): 
   - `pin.pauseAll`, `pin.unpauseAll`, `pin.tellActive`, `pin.tellWaiting`, `pin.tellStopped`, `pin.getGlobalStat`
5. **Result Management** (`test_05_cleanup`): 
   - `pin.purgeDownloadResult`, `pin.removeDownloadResult`

> **Note:** `pin.addTorrent` is currently a stub in the backend and is excluded from testing. `pin.shutdown` is also excluded so the test suite can be run repeatedly without needing to manually restart the server.

### CLI Tests (`tests/test_cli.py`)
This suite tests the direct binary execution (`pincer [URL] [OPTIONS]`) without starting the daemon. It covers:
1. **Help & Version**: Output of `--help` and `--version`.
2. **Direct Downloads**: Downloading a file directly via CLI arguments (`--out`, `--split`, etc.) and verifying the file is written to disk successfully.

## How to Run the Tests

The easiest and recommended way to run the entire test suite is using the automated test runner script. If you need to run specific suites individually, you can also execute them manually.

### The Automated Test Runner (`tests/run_tests.py`)

Pincer includes an all-in-one test runner script at `tests/run_tests.py`. This script manages the entire lifecycle of CI verification, compilation, execution, and cleanup.

#### What the Runner Does:
1. **Conflicting Process Check**: Automatically scans for and terminates any pre-existing running Pincer processes to prevent port bind conflicts on `6842`.
2. **CI Verification Checks**:
   - Runs `cargo fmt --all -- --check` (attempts to auto-format using `cargo fmt` if check fails).
   - Runs `cargo check --all-targets` to verify code compiles.
   - Runs `cargo clippy --all-targets -- -D warnings` to verify zero lint warnings.
3. **Build Stage**: Builds the debug binary (`cargo build`).
4. **Session Cleanup**: Removes any stale/leftover session file from `~/.pincer/pincer.session`.
5. **CLI Tests**: Runs `tests/test_cli.py` to verify direct download functionality.
6. **Background Server**: Spawns Pincer in the background, waiting for it to spin up and bind the socket.
7. **RPC Tests**: Runs `tests/test_rpc.py` to test websocket JSON-RPC methods end-to-end.
8. **Server Shutdown**: Properly terminates the background server.
9. **Cleanup Prompt**: Asks if you want to clean up temporary test files in `tests/temporary`.

#### How to Run the Runner:
Ensure you have the `websockets` dependency installed, then execute the script:
```bash
# Ensure dependency is installed
pip install websockets

# Run the test suite
python3 tests/run_tests.py
```

---

### Running Tests Manually

If you prefer to run CLI or RPC tests independently, you can follow these steps:

### 1. RPC Tests (`tests/test_rpc.py`)
To run the RPC tests manually, the Pincer server must be running in a separate process.

1. Open a terminal and start the Rust backend from the project root:
   ```bash
   cargo run
   ```
   The server should log that it is listening on `ws://0.0.0.0:6842/jsonrpc` (or another port if started with `--port`).

2. Open a separate terminal and execute the Python test suite:
   ```bash
   # Run the RPC tests
   python3 -m unittest tests/test_rpc.py
   ```

### 2. CLI Tests (`tests/test_cli.py`)
The CLI tests do not require the server to be running. The test script will automatically run `cargo build` to compile the binary and then test the executable directly.

```bash
# Run the CLI tests
python3 -m unittest tests/test_cli.py
```

## What to Expect
When you run the test suite, Python's `unittest` runner will execute the tests asynchronously. 

You should see an output indicating success:
```text
......
----------------------------------------------------------------------
Ran 6 tests in 0.234s

OK
```

If a test fails, the runner will output a traceback detailing which RPC method failed. Common failure reasons include:
- The server isn't running (Connection Refused).
- The server returned an `RPCError` (e.g., invalid parameters).
- The returned JSON structure did not match what the test expected (e.g., missing a `result` key).

## Adding New Tests
When new RPC methods are implemented, they should be added to `tests/test_rpc.py`.
- Group related features into a new or existing asynchronous test method (`async def test_*`).
- Use the provided `self.rpc_call(method, params)` helper method to send the payload and await the JSON response.
- Use standard assertions like `self.assertEqual()`, `self.assertIn()`, and `self.assertIsNotNone()` to validate the server's responses.
