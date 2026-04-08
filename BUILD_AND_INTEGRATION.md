#  Pincer Build & Integration Guide

This document outlines how to compile the Pincer Rust engine into a standalone binary and eventually integrate it as the core download engine for Sluice. 

## 1. Prerequisites (Installing Rust)

Currently, your Mac environment needs the Rust toolchain to compile Pincer.

1. Open your terminal and run the official Rust installer:
    ```bash
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    ```
2. Press `1` to proceed with the default installation.
3. Reload your terminal profile to ensure the `cargo` command is available:
    ```bash
    source $HOME/.cargo/env
    ```
    *(Or simply close and reopen your terminal).*

## 2. Building the Binary

Once Rust is installed, you can compile Pincer locally. Ensure you are inside the `pincer` directory:

```bash
cd /Users/mac/Public/ArchitectureChange/pincer
```

### For Development (Fast Compile)
To run the server locally to debug and test WebSocket connections:
```bash
cargo run
```

### For Production (Max Performance & Binary Creation)
To create the highly optimized standalone binary that you will package with Sluice:
```bash
cargo build --release
```

Once the compilation finishes, your executable will be located here:
`target/release/pincer`

---

## 3. Integrating with Sluice (Future Steps)

When you are ready to replace Aria2 in Sluice with Pincer, follow these steps:

### A. Bundling the Binary
1. Copy the compiled `target/release/pincer` executable.
2. Drag and drop it into your Sluice Xcode project (make sure it's added to the target bundle).

### B. Launching Pincer via Swift
Instead of spawning the `aria2c` process, you will spawn the `pincer` process. In your Sluice `App` or `ViewModel` lifecycle, you use Swift's `Process` (sometimes encapsulated in your existing daemon manager) to launch it:

```swift
let pincerProcess = Process()
// Find where Pincer is bundled in your App
if let executableURL = Bundle.main.url(forResource: "pincer", withExtension: nil) {
    pincerProcess.executableURL = executableURL
    // No cumbersome torrent or daemon arguments needed!
    try? pincerProcess.run()
}
```

### C. Re-targeting the Swift JSON-RPC 
Finally, change the `RPCMethod` enum in `Aria2RPCClient.swift` back to the `pin.` namespaces when the backend is ready:

```swift
enum RPCMethod: String {
    case addUri = "pin.addUri"
    case pause = "pin.pause"
    case unpause = "pin.unpause"
    case remove = "pin.remove"
    // ...
}
```
Because Pincer uses the exact same `TaskStatus` JSON structures your models expect, everything from the progress bars to the multithreaded pause/resume will automatically start functioning using the new, safer Rust memory constraints.
