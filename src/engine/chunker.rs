//! Byte range calculation & division
//!
//! ### Architectural Overview
//! - **What it does**: Partitions a file's total byte length into contiguous byte ranges for parallel multi-worker downloading, accounting for existing checkpoint progress.
//! - **How it does**: Divides `total_size` by thread count `N`, adjusts the final chunk to cover remainders, offsets individual worker starting positions by `worker_progress[i]`, and returns `RangeChunk` descriptors.
//! - **Where it comes from**: Called by `engine::DownloadTask::start()` before spawning worker threads.
//! - **Where it leads to**: Supplies segmented `RangeChunk` descriptors consumed by `engine::DownloadWorker::spawn()`.

/// Segmented byte range descriptor allocated to a specific worker thread.
#[derive(Debug, Clone)]
pub struct RangeChunk {
    /// Zero-based identifier of the worker thread.
    pub worker_id: usize,
    /// Absolute byte offset where this worker should resume downloading.
    pub start: u64,
    /// Absolute ending byte offset for this segment (inclusive).
    pub end: u64,
}

/// Computes byte-range partitions for parallel multi-threaded downloading.
pub struct RangeChunker;

impl RangeChunker {
    /// Partitions `total_size` across `threads` workers, taking existing `worker_progress` into account.
    ///
    /// Algorithmic breakdown:
    /// 1. Compute nominal chunk size: `total_size / threads`.
    /// 2. For worker `i`, initial segment bounds are `[i * chunk_size, (i + 1) * chunk_size - 1]`.
    /// 3. The last worker's segment is extended to `total_size - 1` to include any remainder bytes.
    /// 4. Apply checkpoint offset: `start = start_bound + worker_progress[i]`.
    /// 5. If `start > end_bound`, the chunk is already fully downloaded and skipped.
    pub fn calculate_chunks(
        total_size: u64,
        threads: usize,
        worker_progress: &[u64],
    ) -> Vec<RangeChunk> {
        let mut chunks = Vec::new();
        if threads == 0 || total_size == 0 {
            return chunks;
        }

        let chunk_size = total_size / threads as u64;

        for i in 0..threads {
            let start_bound = i as u64 * chunk_size;
            let end_bound = if i == threads - 1 {
                total_size - 1
            } else {
                (i as u64 + 1) * chunk_size - 1
            };

            let progress = worker_progress.get(i).copied().unwrap_or(0);
            let start = start_bound + progress;

            if start <= end_bound {
                chunks.push(RangeChunk {
                    worker_id: i,
                    start,
                    end: end_bound,
                });
            }
        }

        chunks
    }
}
