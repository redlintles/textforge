# Processor parity through the portable API

`api::portable::PortableProcessor` is the full, owned processor facade. The earlier
`PipelinePlan::compile()` / `CompiledPipeline::process()` convenience path remains
available for independent immutable in-memory executables. Applications that need
registration, persistence, observers, streams or file batches use the processor
facade. Existing fluent Rust contracts are unchanged.

## Coverage matrix

| Core entry point | Portable entry point / representation |
| --- | --- |
| `new`, `Default` | `PortableProcessor::new`, `Default` |
| `create_pipeline` | `create_pipeline` returns an independent `PipelinePlan`; `register(&plan)` validates and registers it |
| `add_transform` | `add_transform(Vec<InstructionSpec>)`, atomically validated |
| `remove_transform`, `transform_exists`, `show_transforms` | Same names; `list_transforms` additionally returns owned IDs |
| `get_transform_vec` | Same name, returns independent editable `InstructionSpec` snapshots |
| `get_text_transform_vec` | Same name, delegates the core text representation |
| cloning through `get_transform_vec` + `add_transform` | `clone_pipeline`, or snapshot/edit/register |
| `get_pipeline_from_file`, `write_pipeline_to_file` | Same names with `FilePath` |
| `read_from_bytecode_file`, `write_to_bytecode_file` | Same names with `FilePath`, under `bytecode` |
| `run_transform`, `process_all` | Same names; borrowed-result internals become owned returned strings |
| `process_single` | Same name with `InstructionSpec` |
| `process_all_with_debug`, `process_single_with_debug` | Same names, preserving native console output and core execution |
| `process_file` | Same name with owned-path objects |
| `process_batch` | Same name with `Vec<FileTask>`; supports different pipeline IDs per task |
| `process_batch_in_pool` | Same name with reusable `BatchPool` |
| `process_all_with_watchers` | Same name with `Observers` and report `FilePath`, under `watchers` |
| `process_all_buffered`, `process_single_buffered` | Same names with `PortableBuffer`, under `buffered`; the single-instruction method retains the core textual-instruction argument |
| `process_file_buffered` | Same name with `FilePath` and `BufferOptions` |
| `process_batch_buffered` | Same name with `Vec<BufferTask>`; returns ordered per-task owned buffer results |

This is functional parity, not exposure of internal trait objects or identical
Rust signatures. Invalid raw token vectors are represented by descriptions and
checked before registration. The instruction registry and the shared tree rules
remain authoritative. A snapshot can be inspected, changed and registered without
private parser types. No processor handle or raw pointer is exported.

The facade delegates execution and persistence to the existing processor. This
includes existing feature-dependent execution choices, stdout behavior, path and
extension checks, text serialization constraints, watcher observation timing,
result ordering and core error codes. It does not implement another pipeline
engine or silently repair/replace existing file formats. In particular, pipeline
file writers retain the core's destination-path validation (which currently
expects an existing path); document output methods follow their core behavior.

## Typical workflow

```rust
use textforge::api::portable::{Argument, InstructionSpec, PortableProcessor};
let mut processor = PortableProcessor::new();
let id = processor.add_transform(vec![
    InstructionSpec::new("atb".into(), vec![Argument::Text("🍌".into())]),
]).unwrap();
let snapshot = processor.get_transform_vec(&id).unwrap();
let duplicate = processor.add_transform(snapshot).unwrap();
assert_eq!(processor.process_all(&duplicate, "é").unwrap(), "🍌é");
processor.remove_transform(&id).unwrap();
assert!(!processor.transform_exists(&id));
```

## Files, pools and optional features

`FilePath` owns a `PathBuf` internally and exposes checked text, Unix-byte and
Windows-wide constructors/exporters. Non-UTF-8 native names are never routed
through lossy text conversion; asking for another OS's representation returns a
conversion error. `to_text()` returns `None` when there is no exact Unicode text
representation. Adapters marshal path values; the shared layer performs native
conversion. Both byte and wide branches are covered by platform-gated tests.

`FileTask` owns input/output paths and a pipeline ID. `BatchPool` owns a reusable
Rayon pool. Tasks retain input order and individual errors, and a failed task does
not prevent subsequent independent tasks. The layer neither changes the global
pool nor claims a global memory limit.

`features()` returns a concrete `Features` record for capability discovery.
Bytecode, watchers and buffered methods use the same Cargo features as the core.
A future adapter requiring every capability enables the production features:

```toml
textforge = { version = "0.1", features = ["bytecode", "watchers", "buffered"] }
```

`test_access` is not needed by production adapters. Feature-off builds still
support all non-optional processor methods and owned instruction construction.

## Custom observers: runtime invocation only in the adapter

`Observers` handles dynamic registration names, replacement, scheduling, reset,
manual observation and JSON report export. `schedule_all` represents the core's
only currently implemented `ExecutionWindows::All`. `set_builtin` exposes all
current core default watcher functions, discoverable through `builtin_names()`.

For custom callbacks, the adapter implements the small `Observer` interface (or
provides a boxed closure). It receives an owned `Observation` and returns a string
or error string. The core still decides when to observe, how to associate steps,
how to order names, and how to publish/reset the report.

```rust
use textforge::api::portable::{Observation, Observers};
let mut observers = Observers::new();
observers.set("custom-🍌".into(), Box::new(|context: Observation| {
    Ok(context.current.chars().count().to_string())
}));
observers.schedule_all("custom-🍌").unwrap();
```

Callbacks execute synchronously on the calling thread. `Observers` remains
thread-confined, like the core's `Rc`-based watcher list. Adapters must enter their
foreign runtime to invoke its function and convert foreign exceptions to the
callback error result; that runtime-specific invocation cannot be implemented by
a language-neutral Rust layer. They must not reenter a mutably borrowed processor.
All domain scheduling and report handling stay in the shared layer.

A callback error aborts the run with `ErrorStage::Callback`, code 27 and the
callback's error message/name. It does not publish a partial report or overwrite
an existing report. As in the core, successful runs reset accumulated results;
failed runs may retain earlier completed observations until explicit `reset()`.
The public core `WatcherList` methods retain their signatures. Its private keys
now own reference-counted names; a private fallible bridge enables dynamic names
and callback-error propagation without leaking static strings or changing the
JSON schema. Existing built-in watcher identity/optimization stays available for
existing native registrations.

## Buffered documents and foreign streams

`BufferOptions` owns the spill directory and a checked `u64` threshold.
`PortableBuffer` wraps the existing UTF-8 spool, exposing text/file/reader input,
byte length, emptiness, spill state, explicit spill, ranged reads, file output,
stream output and consuming full-string export. No implicit materialization is
introduced by the wrapper. `BufferTask` supplies owned IDs and input paths for
buffered batches; successful batch outputs remain spilled as in the core.

`StreamReader` and `StreamWriter` are synchronous owned-value callback hooks.
The reader returns at most the requested bytes, with empty data indicating EOF;
the writer returns the accepted count. Private `ReaderBridge` and `WriterBridge`
implement standard Rust I/O for the existing spool code. The layer checks counts,
preserves UTF-8 validation across partial reads, handles partial writes and maps
errors through the core. Adapters only invoke the stream object and marshal bytes.
Stream failures are reported with the core buffer I/O codes at execution stage.

Spill thresholds remain placement choices, not a hard global cap. Existing
transforms that require materialization still do so. This parity layer does not
change the experimental buffered processor's execution/storage semantics.

## Errors and verification

Persistence operations add `ErrorStage::Persistence`; observer errors add
`ErrorStage::Callback`. Existing numeric core codes are retained. Conversion,
validation and ordinary execution stages remain as documented in the
[portable API guide](portable-api.md).

[Processor parity tests](../tests/portable_processor.rs) exercise registry
lifecycle/snapshots, single/debug execution, text and bytecode persistence,
multiple-pipeline file batches/pools, native paths, dynamic/custom/default
observers, callback failure/report preservation, buffered methods, UTF-8 chunks,
short writes and invalid stream counts. They use only production public paths.
The existing [portable tests](../tests/portable.rs) cover construction semantics,
validation, ownership and independent execution.

A newly exposed core bug is fixed separately: spilled buffers now return EOF for
reads at `u64::MAX` or beyond the document, consistently with memory-backed data.
The regression is tested directly in the core and through the portable wrapper.

When adding a processor capability in the future, extend this matrix, the shared
facade and parity tests before adding language-specific bindings. Adapters still
need entry points for new capabilities; they should not implement domain logic.

### Source references

- [Portable processor and task/pool/capability records](../src/api/portable/processor.rs).
- [Owned native paths](../src/api/portable/path.rs).
- [Observer callback bridge and built-ins](../src/api/portable/watchers.rs).
- [Buffered documents and I/O hooks](../src/api/portable/buffered.rs).
- [Core processor](../src/api/processor.rs).
- [Core watcher list](../src/watchers/mod.rs).
- [Core buffer](../src/api/text_buffer.rs).
