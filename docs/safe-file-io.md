# Safe file processing and bounded batch concurrency

## Why mmap was removed

The previous `process_batch` mapped each input, validated UTF-8, copied the complete document into an owned string and kept the mapping alive throughout transformation. `process_file` also held a mapping while its pipeline ran. Neither public method guaranteed that another process would leave the mapped file unchanged. This made the unsafe mapping precondition unsuitable for these safe file-path APIs.

The replacement removes both direct unsafe blocks and the `memmap2` dependency. The library now forbids unsafe code at compile time. No dependency was added and no existing method signature or file extension changed.

## Read path and allocation ownership

Both ordinary file paths use the shared `read_utf8_file` helper in `src/api/processor/file_io.rs`. It opens the file once, reads through `File::read_to_end`, then validates with `String::from_utf8`. Rust's File implementation uses size information to guide capacity reservation when available. The Vec allocation becomes the String allocation; validation does not copy the byte buffer. An extra BufReader would add buffering to a read that already collects the complete file.

The default whole-document engine receives an owned string directly, including from `process_file`, so native operations that consume ownership can reuse it. Opening, reading and UTF-8 failures retain their distinct TextForgeError categories; transformations finish before output creation. Empty inputs need no special mapping branch. Reading completes and the file handle closes before any output is opened, including when input and output paths are equal.

With `buffered` enabled, `process_batch` already used TextBuffer without mmap. That existing path is preserved. `process_file` retains its feature-dependent `process_all` dispatch and String-returning behavior after a safe read. For initial spooling and file-oriented output, use the existing explicit buffered APIs; those APIs and their allocation policies are unchanged by this work.

A safe read protects Rust memory safety when another process changes the file. It does not guarantee a consistent filesystem snapshot. Output writes also retain their previous non-atomic behavior: a write failure after output creation can leave a partial output. Concurrent tasks must use independent output paths; ordering of conflicting writes was never guaranteed.

## Reusable Rayon pools

`process_batch` continues using Rayon's current pool. The new additive `process_batch_in_pool` method accepts a caller-owned pool and delegates through `ThreadPool::install`. It works with both engine configurations, preserves result order and individual task errors, and leaves global pool configuration untouched.

```rust
use textforge::api::processor::TextForgeProcessor;
use std::path::Path;

fn run(
    processor: &TextForgeProcessor,
    pipeline_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Construct once and keep this pool for subsequent calls.
    let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;
    let results = processor.process_batch_in_pool(
        &pool,
        vec![(Path::new("input.txt"), pipeline_id, Path::new("output.txt"))],
    );
    for result in results {
        result?;
    }
    Ok(())
}
```

No automatic RAM heuristic or per-call pool creation was added. Worker count bounds concurrent task execution for an ordinary batch, not total bytes. File sizes, expanding transforms, contexts and materialized fallback results still determine each task's memory use. Concurrent callers sharing a pool share its capacity. A future byte-budget scheduler would need expansion estimates and explicit oversized-document behavior; a strict memory bound would additionally require compatible streaming instructions or checked allocation limits.

## Tests, benchmarks and commit boundaries

`tests/safe_file_io.rs` exercises the public methods with every feature configuration. A private test instruction records the actual Rayon pool width from inside execution. This checks pool dispatch rather than merely testing ThreadPoolBuilder itself.

Run the complete AGENTS.md checklist with `python3 scripts/validate_safe_file_io.py PHASE`. It retains failures and compressed logs in `docs/benchmarks/safe-file-io/`. Existing repository-wide formatting/Clippy issues and private-parser doctest failures are recorded explicitly; none are hidden by lint allowances or ignored tests.

Run `python3 scripts/benchmark_safe_file_io.py` for the release comparison. It regenerates deterministic fixtures in `data/safe-file-io`, pipelines in `pipelines/safe-file-io`, and compares the saved pre-removal source with the current source. The generated report documents timing scope, warm-cache limitations and independent output verification.

The source archives contain the complete worktree versions measured, including earlier uncommitted optimizations present equally on both sides. The safe-read commits intentionally isolate this change from that earlier work. No publishing, pushing, version bump or unrelated formatting changes are part of the removal.
