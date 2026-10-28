# Experimental TextBuffer execution

Branch: `codex/test-tempfile`, based on the four crash-fix commits ending at `3b5d55a`. This is an opt-in experiment (`buffered` feature), not a stable ABI or a promise of bounded memory for every token.

## Step 1: owned storage

`src/api` owns the public TextBuffer and TextBufferOptions types. A buffer is one valid UTF-8 document, stored in tempfile::SpooledTempFile. Options explicitly name the per-buffer threshold and spill directory. Choose a disk-backed directory: a tmpfs directory still consumes RAM.

The buffer is consumed by into_string and remains owned until then. Byte-range reads and copying to a writer do not require materializing the document. There is no implicit str conversion, raw pointer API, public tempfile handle, or Clone. These choices support a future opaque-handle FFI with explicit release, length, bounded byte reads, and fallible string export. No extern C functions, unsafe code or cdylib configuration are introduced.

The threshold controls placement, not a hard memory budget: tempfile allocations, input/output overlap, OS page cache and concurrent work also consume resources. Existing APIs and their return types remain unchanged. tempfile was already a dependency; the new feature enables it for runtime use without changing its version.

Validation and subsequent execution/benchmark stages are recorded below.

## Step 2: whole-document executor

Added inherent process_all_buffered, process_single_buffered, process_file_buffered and process_batch_buffered methods to the processor through src/api. Existing methods still return String and existing trait implementations remain compatible. The opt-in InstructionMethods extension has a default compatibility path, so third-party tokens need not implement it immediately.

One GlobalExecutionContext lives across all instructions for each document. Native atb/ate/rpt/null/end paths never split the pipeline into separate chunk executions. All other tokens use the existing whole-string implementation; this intentionally preserves current behavior, including known bugs, and can still allocate memory proportional to the entire document. Nested blocks/conditionals/iterations currently take that compatibility path as well.

Batch execution is sequential in this experiment and forces completed results to disk before retaining them. The legacy parallel batch API is untouched. This avoids accumulating a threshold-sized RAM spool for every returned result, but does not cap descriptors, disk usage or legacy fallback peaks. Debug and watcher variants are intentionally not added until their existing semantic divergences are resolved; their existing APIs remain unchanged.

## FFI boundary decisions

The new public results are owned objects under src/api; Rust layouts and generics are not a C ABI. Future bindings should use opaque processor/options/task/buffer handles, typed status codes, u64 byte lengths, checked platform-size conversions, and explicit release functions. Buffer export should be pointer-plus-length or bounded read calls, not an unconditionally NUL-terminated C string: documents may contain embedded NUL. Reads mutate an internal cursor, so bindings must serialize calls on the same handle. No pointer into a spool allocation is borrowed across calls or migration to disk.

Processing and into_string consume the input buffer even when they fail. A future FFI must invalidate/retire the consumed input handle on both success and failure; it must not expose a dangling handle or release it twice. Options/task descriptors have separate lifetimes.

## Implementation validation

- Eight storage unit tests passed (including 90 combinations of bounded operations, one-byte UTF-8 reads, I/O failures and cursor independence).
- Ten buffered integration tests passed, including 42 fixed pipeline/storage combinations, 100 generated mixed pipelines, all-token reference execution, bytecode loading, cross-boundary regex matching, batch isolation, cleanup and bounded byte export.
- All five new src/api doctests also passed with only the buffered feature, without test_access.
- Full test_access suite: **971 passing tests**, including 121 doctests.

Full pre-commit checklist (offline dependency cache):

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

The unchanged baseline failures remain: repository formatting, Clippy diagnostics, and two doctests importing private parser modules without test_access. No new Clippy diagnostic points to the added buffer/executor/test modules. No publication, version bump, FFI implementation or changes to current extension/opcode contracts were made.

## Reproducible probe and load results

`examples/buffered_probe.rs` runs the legacy and buffered APIs against the same registered pipeline. `scripts/benchmark_buffered.py` generates inputs and checks outputs with independent SHA-256 references. Raw measurements are in [tempfile-results.json](experiments/tempfile-results.json).

Environment: Linux, Ryzen 5 4600G (6 cores/12 threads), about 27 GiB RAM, Rust 1.98.0; release build with `buffered`, tempfile 3.27.0; spill directory on ext4, not tmpfs. Spool threshold 1 MiB unless stated otherwise. Peak RSS comes from /usr/bin/time; 2 GiB address-space cap normally, 96 MiB in pressure cases; child process groups have a 45-second timeout and core dumps disabled.

Time includes startup, pipeline loading, input validation, transformations and output writes. Page caches were not flushed, CPU was not isolated, and some initial runs overlapped the test matrix. These are local observations, not production performance guarantees. RSS excludes much of the kernel filesystem page cache: low process RSS does not prove equally low total host memory pressure.

### Whole-document native operations

The identity and prefix/suffix cases below use three executions per mode. Prefix/suffix is `atb "<";` followed by `ate ">";`. Each output hash matched the reference.

| Input | Pipeline | Legacy median time | Buffered median time | Legacy median RSS | Buffered median RSS |
|---|---|---:|---:|---:|---:|
| 64 MiB | noop | 0.155 s | 0.129 s | 132.6 MiB | 5.9 MiB |
| 64 MiB | wrap | 0.164 s | 0.182 s | 132.5 MiB | 5.9 MiB |
| 256 MiB | noop | 0.603 s | 0.979 s | 516.6 MiB | 5.8 MiB |
| 256 MiB | wrap | 1.149 s | 1.311 s | 516.3 MiB | 5.8 MiB |

For these native operations, disk-backed output made process memory largely independent of document size over the tested range. The tradeoff is extra I/O and potentially longer execution. Ingestion currently copies a snapshot into the spool; it does not adopt a mutable external input file as internal storage.

### Expansion, materialization, fallback and batches

The following are individual observations, not three-run medians. `repeat` is rpt 4 on 64 MiB (256 MiB output). `fallback` is regex replacement of alice followed by uppercase on 64 MiB; both tokens currently materialize through the compatibility path. Batches use 16 MiB per task and prefix/suffix.

| Workload | Mode | Time | Peak RSS |
|---|---|---:|---:|
| repeat, batch=1 | legacy | 1.070 s | 324.3 MiB |
| repeat, batch=1 | buffered | 1.093 s | 5.9 MiB |
| repeat, batch=1 | buffered-string | 1.418 s | 261.3 MiB |
| fallback, batch=1 | legacy | 0.490 s | 199.5 MiB |
| fallback, batch=1 | buffered | 0.443 s | 135.9 MiB |
| fallback, batch=1 | buffered-string | 0.447 s | 136.0 MiB |
| wrap, batch=4 | legacy-batch | 0.072 s | 186.3 MiB |
| wrap, batch=4 | buffered-batch | 0.135 s | 6.0 MiB |
| wrap, batch=16 | legacy-batch | 0.173 s | 483.7 MiB |
| wrap, batch=16 | buffered-batch | 0.510 s | 5.8 MiB |

Explicit conversion of the repeated 256 MiB result to String increased RSS back to roughly the result size. The fallback path still consumed memory proportional to the complete document, even though this particular two-stage workload used less RSS than the reference executor. It is not bounded-memory regex execution.

Batch results were correct for all 4 and 16 outputs. Buffered batch is sequential and retains completed results on disk; legacy batch is parallel. Their timings therefore compare different scheduling policies as well as storage. The experiment intentionally favors low memory over parallel throughput; it is not a fair claim of a faster scheduler.

The Unicode load preserved composed emoji, combining marks, CR/LF where present, and byte ordering under native prefix/suffix. A 1 MiB input with an 8 MiB spool threshold stayed in memory, showing that small results do not have to go to disk.

### Resource pressure

Under a 96 MiB virtual address-space limit:

- Prefix/suffix on 64 MiB: legacy execution aborted on allocation; buffered execution completed with a correct hash and about 6 MiB RSS. Explicitly materializing that approximately 64 MiB result still fit the allowance in this case.
- Repeat 4 on 64 MiB: legacy execution returned an allocation error; buffered execution produced the entire 256 MiB result with a correct hash and about 6 MiB RSS. Explicit String conversion then returned a recoverable materialization error, as expected under this allowance.
- A permanent Linux regression test separately produces and drains a 128 MiB result in a subprocess limited to 96 MiB, proving this native path does not silently materialize its result.

There were **44 measured executions: 41 completed outputs with correct hashes, zero mismatches, and three expected failure exits in the pressure experiments**. These failures are not reported as successful processing.

### Reproduction

```sh
cargo build --release --features buffered --example buffered_probe
python3 scripts/benchmark_buffered.py \
  --probe target/release/examples/buffered_probe \
  --directory /path/on/disk/textforge-buffered-benchmark
cargo test --features buffered --lib --tests
cargo test --features buffered,bytecode --lib --tests
cargo test --features test_access
```

The benchmark uses Linux resource limits and /usr/bin/time, creates several hundred MiB of inputs plus output files, and must point at a dedicated writable directory. `--pressure-only` reuses a previous run directory and appends the three expansion-pressure observations. It does not regenerate the normal measurements.

### Remaining limits and decisions

- Only atb, ate, rpt, null and end have native buffer execution. Other instructions, including complex nested constructs, may allocate full strings. This is an incremental experiment, not full streaming coverage.
- Existing scalar-indexed Unicode behavior and known conditional/debug/watcher issues are not redefined here. Native buffer copying preserves bytes; it does not implement a new grapheme-indexing model.
- Per-buffer thresholds do not enforce a shared process/batch budget. File descriptor count, disk usage, metadata/parameter sizes, OS cache and legacy fallback allocations remain uncapped.
- Disk exhaustion or inaccessible spill directories can still produce errors. Large results returned as handles remain on disk until callers consume or drop them. The temporary file is not a durable output artifact.
- tempfile uses allocations internally; the threshold is not a proof that every allocation is fallible. No guarantee against OS termination under overcommit is made.
- Existing APIs still use their original executor, including mmap in old file/batch paths. The experimental buffered file/batch paths do not use mmap. Removing the dependency entirely is a separate compatibility/performance task.
- No FFI ABI, debug/watchers buffer variants or global scheduler is implemented. These require explicit design beyond proving the owned-buffer execution model.

### Final validation

The buffered-only and buffered+bytecode library/integration test configurations passed. The pressure subprocess regression passed. New API doctests passed without test_access, and the example compiled in release. Clippy on the example completed with only the repository's existing library warnings.

Full pre-commit checklist after adding the probe and pressure regression:

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

Final combined test_access count: **972 passing tests**. Pre-existing format, Clippy and two normal-feature doctest failures remain documented above.
