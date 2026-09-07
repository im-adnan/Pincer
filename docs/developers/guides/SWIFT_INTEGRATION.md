# Swift Integration Guide

Step-by-step guide to integrating Pincer Engine into a macOS Swift/SwiftUI application.

> **See also**: [API Overview](../api/README.md) · [Connection & Auth](../api/CONNECTION.md) · [Events](../api/EVENTS.md)

---

## Overview

Pincer Engine is designed to run silently as a backend daemon for frontend graphical user interfaces, such as macOS applications built with Swift/SwiftUI. 

To integrate Pincer Engine's capabilities (like FTP/SFTP support, multi-source Metalink fallback, auto-format conversion, etc.) into your UI application, follow this standardized 4-step architecture:

---

## Step 1: Spawn the Engine as a Background Daemon

Bundle the compiled `pincer` binary inside your macOS App bundle. When your app launches, use `Process` (NSTask) to spawn the engine in daemon mode (`-D`) on a specific port.

```swift
let task = Process()
task.executableURL = Bundle.main.url(forResource: "pincer", withExtension: nil)
// Enable RPC daemon on port 6800, using the background detached flag
task.arguments = ["--enable-rpc=true", "--rpc-listen-port=6800", "-D"]
try? task.run()
```

---

## Step 2: Connect to the WebSocket RPC Server

Once spawned, Pincer runs a lightweight WebSocket server locally. Establish a persistent WebSocket connection from your Swift app to send commands and receive real-time updates.

```swift
let url = URL(string: "ws://localhost:6800/jsonrpc")!
let session = URLSession(configuration: .default)
let webSocketTask = session.webSocketTask(with: url)
webSocketTask.resume()
```

---

## Step 3: Trigger Features via JSON-RPC

Instead of executing complex CLI commands, your UI will trigger Pincer's features by sending standardized JSON-RPC payloads over the WebSocket. 

All of Pincer's features are invoked similarly. For example, to trigger the Metalink multi-source download feature:

```swift
// Example: Triggering a Metalink Task
let payload: [String: Any] = [
    "jsonrpc": "2.0",
    "id": UUID().uuidString,
    "method": "pin.addMetalink",
    "params": [
        "PD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0i...", // Base64 encoded payload
        ["dir": "/Users/Shared/Downloads", "split": "16"] // Engine options
    ]
]
let jsonData = try! JSONSerialization.data(withJSONObject: payload)
webSocketTask.send(.data(jsonData)) { error in ... }
```

*(Whether you are using `pin.addUri` for FTP links, or `pin.changeGlobalOption` to throttle speeds, the JSON-RPC interface remains identical).*

---

## Step 4: Map Introspection Models and Listen to Events

Pincer streams real-time status events back to your WebSocket. You do not need to poll manually. Create Swift `Codable` structs that map to Pincer's introspection models (e.g., `PincerFile`, `PincerUri`, `TaskStatus`) to cleanly decode these incoming JSON payloads.

```swift
// Ensure you catch Pincer's standardized events to update your UI:
// - pin.onDownloadStart
// - pin.onDownloadComplete
// - pin.onDownloadError (e.g. emitted if a checksum validation fails!)

webSocketTask.receive { result in
    switch result {
    case .success(.string(let text)):
        // Decode the JSON-RPC response and update your UI progress bars
        let response = try? JSONDecoder().decode(PincerRPCResponse.self, from: text.data(using: .utf8)!)
    case .failure(let error):
        print("WebSocket Error: \(error)")
    }
}
```

---

## Summary

By keeping the heavy lifting inside the Pincer Rust engine, your Swift frontend can remain incredibly lightweight—simply sending JSON-RPC commands and painting the UI based on the incoming WebSocket event stream.

For the full list of available events, see [Events](../api/EVENTS.md). For all available RPC methods, see the [API Overview](../api/README.md).
