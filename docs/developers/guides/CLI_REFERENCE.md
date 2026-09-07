# CLI Reference

Complete reference for using Pincer as a standalone command-line download tool.

> **See also**: [User Guide](../../USER_GUIDE.md) · [Session Persistence](SESSION_PERSISTENCE.md) · [API Overview](../api/README.md)

---

## Basic Usage

For quick direct downloads without using the RPC server:

```bash
./target/release/pincer "https://example.com/file.zip"
```

This will download the file to the current directory using multiple threads by default.

---

## Argument Reference

| Flag | Long Form | Description | Default |
| :--- | :--- | :--- | :--- |
| `-s` | `--split` | Number of concurrent threads/connections (1-99). | `1` |
| `-d` | `--dir` | Target directory for the download. | Current Dir |
| `-o` | `--out` | Custom output filename. | From URL |
| `-f` | `--format`| Target format to convert the downloaded file to. | N/A |
| `-l` | `--log` | Enable detailed logging. | `false` |
| `-p` | `--port` | RPC server port. | `6842` |
| `-D` | `--daemon`| Run Pincer Engine as a detached background daemon. | `false` |
| `-h` | `--help` | Print help information. | N/A |

### Planned CLI Flags

The CLI parser (using `lexopt`) will be expanded to support additional flags for users who want to use Pincer exclusively from the terminal without the RPC server:

*   `-v` or `--version`
*   `-V` or `--check-integrity`
*   `-j` or `--max-concurrent-downloads`
*   `-c` or `--continue` (Resumption)

---

## Examples

**High-Speed 16-Thread Download:**
```bash
./pincer "https://example.com/movie.mp4" -s 16 -d ~/Downloads -o holiday_video.mp4
```

---

## Thread Control

### Technical Architecture & Threads

*   **Rust-Native Performance**: Built on `tokio` for non-blocking I/O and zero-allocation disk writes.
*   **Dynamic Thread Scaling**: Supports up to **99 threads** per task. The engine automatically splits the file into equal byte-ranges and assigns a dedicated worker to each.
*   **Range Support Detection**: If a server does not support `Accept-Ranges`, Pincer gracefully falls back to a single thread to ensure data integrity.

In **Pincer**, download threads are dynamically controlled **via the JSON-RPC interface** when adding or modifying a task using options like `split` (which defines the number of connections/threads per download) and `min-split-size`.

---

## Worker Log Verification

To verify that Pincer is correctly utilizing multi-threading, you can observe the internal worker logs during a CLI download.

**Example Log Output (`-s 16`):**
```text
🚀 Pincer CLI Mode
URL: https://images.pexels.com/.../photo.jpeg
Threads: 16
-------------------------------------------
[INFO] [Worker 0] Assigned range: 0 - 47938 (47939 bytes)
[INFO] [Worker 1] Assigned range: 47939 - 95877 (47939 bytes)
...
[INFO] [Worker 0] Connection established. Segment range: 0 - 47938
[INFO] [Worker 2] Connection established. Segment range: 95878 - 143816
...
[========================================] 100.00%
✅ Download complete!
```

### Why Use These Features?

1.  **99 Threads**: Maximizes bandwidth utilization on high-latency connections or from servers that throttle single-connection speeds.
2.  **Session Persistence**: Critical for long-running downloads; protects progress against system crashes or engine restarts.
3.  **Advanced CLI**: Allows developers and power users to use Pincer as a drop-in, high-performance download engine.
4.  **Worker Logs**: Provides transparency and allows users to debug connection issues or verify server range support in real-time.
