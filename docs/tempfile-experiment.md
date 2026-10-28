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
