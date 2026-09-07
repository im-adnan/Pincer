# CLI Reference

Complete reference for using Pincer as a standalone command-line download tool.

> **See also**: [User Guide](02-user-guide.md) for quick-start examples · [Session & Resume](guides/01-session-and-resume.md) for persistence details

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

The CLI parser (using `lexopt`) will be expanded to support additional flags:

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

## How Threading Works

*   **Rust-Native Performance**: Built on `tokio` for non-blocking I/O and zero-allocation disk writes.
*   **Dynamic Thread Scaling**: Supports up to **99 threads** per task. The engine automatically splits the file into equal byte-ranges and assigns a dedicated worker to each.
*   **Range Support Detection**: If a server does not support `Accept-Ranges`, Pincer gracefully falls back to a single thread to ensure data integrity.

Download threads are dynamically controlled **via the JSON-RPC interface** when adding or modifying a task using options like `split` and `min-split-size`. See [Configuration](api/04-configuration.md) for all available options.

---

## Verifying Multi-Threading (Worker Logs)

To verify that Pincer is correctly utilizing multi-threading, observe the internal worker logs during a CLI download.

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
