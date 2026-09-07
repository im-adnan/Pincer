# Session Persistence & Resumption

How Pincer automatically saves progress and resumes interrupted downloads.

> **See also**: [CLI Reference](CLI_REFERENCE.md) · [System Methods](../api/SYSTEM.md) · [Configuration](../api/CONFIGURATION.md)

---

## Overview

Pincer includes a built-in persistence layer. All tasks, global options, and progress are saved to a session file.

---

## How It Works

- **Session File**: `pincer.session` (JSON format).
- **Auto-Save**: Triggered on every significant state change (adding a task, pausing, or changing options).
- **Auto-Load**: On startup, Pincer detects the session file and restores all tasks.
- **Resumption**: Interrupted downloads will automatically resume from the last successfully written byte using HTTP Range requests.

---

## Manual Session Control

You can also manage sessions explicitly via the JSON-RPC API:

| Method | Description |
|:-------|:------------|
| `pin.saveSession` | Manually triggers a session save to disk. |
| `pin.getSessionInfo` | Returns the current session ID. |
| `pin.shutdown` | Gracefully saves session and stops the engine. |
| `pin.forceShutdown` | Immediately exits the engine **without** saving. |

For the full system methods reference, see [System Methods](../api/SYSTEM.md).
