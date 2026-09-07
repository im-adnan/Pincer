# Pincer Developer Documentation

This directory contains all technical documentation for contributors, integrators, and maintainers of the Pincer Engine.

---

## 📐 Architecture & Design

| Document | Description |
|:---------|:------------|
| [Architecture](ARCHITECTURE.md) | System design, module layout, concurrency model, and subsystem overview. |
| [Roadmap](ROADMAP.md) | Dependency migration strategy and technical optimization paths. |
| [Feature Status](FEATURE_STATUS.md) | Current implementation status, achieved features, and prioritized backlog. |

---

## 🔌 JSON-RPC API Reference

The complete reference for Pincer's WebSocket-based JSON-RPC 2.0 interface, organized by domain.

| Document | Description |
|:---------|:------------|
| [API Overview →](api/README.md) | Quick-start, connection details, and navigation. |
| [Connection & Auth](api/CONNECTION.md) | WebSocket URL, port configuration, and token authentication. |
| [Task Management](api/TASK_MANAGEMENT.md) | `pin.addUri`, `pin.remove`, `pin.pause`, `pin.unpause`, and more. |
| [Status & Monitoring](api/STATUS_AND_MONITORING.md) | `pin.tellStatus`, `pin.tellActive`, `pin.getGlobalStat`, and more. |
| [Configuration](api/CONFIGURATION.md) | `pin.changeOption`, engine defaults, user prefs, and system keys. |
| [Advanced Features](api/ADVANCED_FEATURES.md) | URL resolution, format conversion, Metalink, FTP/SFTP, speed modes. |
| [Events](api/EVENTS.md) | Real-time WebSocket notification events. |
| [System](api/SYSTEM.md) | `pin.getVersion`, `pin.shutdown`, `system.multicall`, and more. |

---

## 📚 Integration Guides

Practical, task-focused guides for specific use cases.

| Document | Description |
|:---------|:------------|
| [CLI Reference](guides/CLI_REFERENCE.md) | Full CLI argument table, thread control, and worker log verification. |
| [Session Persistence](guides/SESSION_PERSISTENCE.md) | How `pincer.session` auto-save, auto-load, and resumption work. |
| [Swift Integration](guides/SWIFT_INTEGRATION.md) | 4-step guide to embedding Pincer in a macOS Swift/SwiftUI app. |
| [API Comparison](guides/API_COMPARISON.md) | Pincer method mapping vs standard download engine patterns. |

---

## 🧪 Contributing & Testing

| Document | Description |
|:---------|:------------|
| [Contributing](CONTRIBUTING.md) | Code style, SRP rules, PR workflow, and contribution checklist. |
| [Testing](TESTING.md) | Test architecture, automated runner, and manual test instructions. |
| [Build & Release](BUILD_AND_RELEASE.md) | Building from source, release script, and CI pipeline. |
