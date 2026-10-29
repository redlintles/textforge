# Experimental standard-method adapters

Enable `buffered` to select the experimental implementations of the existing
`TextForgeProcessorMethods` methods. Their signatures are unchanged. Without
that feature, the original implementations remain selected.

```sh
cargo build --features buffered
python3 scripts/benchmark_buffered_comparison.py
```

- `process_all`: copies the borrowed input into a `TextBuffer`, runs the buffered
  engine and materializes the final `String`. The caller's input remains in RAM.
- `process_batch`: reads and validates each input through `TextBuffer`, keeps
  Rayon parallel scheduling and ordered per-task results, creates missing output
  directories and writes each result directly to its output file. Tasks continue
  independently after errors. It does not use the experimental sequential
  `process_batch_buffered` scheduler.
- `process_all_with_debug`: records the same complete before/after and block
  information into a spool, printing only after successful pipeline execution.
  It does not keep a complete log in a `String`. Forced-color snapshots still
  materialize one document at a time to preserve nested ANSI reset handling.

Document and debug-log spools use a 1 MiB threshold and `std::env::temp_dir()`.
This is a per-buffer placement threshold, not a process-wide memory limit.
Temporary-file writes can increase CPU and wall time. Resident-memory readings
exclude filesystem page-cache memory; the temporary filesystem may itself be
memory-backed. Existing APIs accepting `TextBufferOptions` remain available when
explicit thresholds or spill directories are needed.

Native buffered operations currently include prefix, suffix, repetition, null
and end. Other operations fall back to full-document materialization, so
fallback-heavy pipelines may not reduce peak RSS for non-debug processing.
String-returning APIs must also allocate their final output; safe allocation
refusal can consequently happen at final materialization rather than during
native buffered repetition.

## Reproducible comparison

`benchmark_buffered_comparison.py` builds original commit `608e4fc` with default
features and the current working source with `buffered`, using the identical
release worker. It archives exact source fingerprints, independent expected
output hashes, 18 debug-log/result equivalence checks, CPU and wall times, and
peak process RSS. Five fresh-process samples per method/scenario/version are
retained after an excluded warm-up, with randomized paired revision order.
Batch has four tasks and uses four Rayon threads in both implementations.
Debug is redirected to `/dev/null` during measurement; terminal rendering is
excluded. Fixtures and pipelines match the earlier comparison.

Results: `docs/benchmarks/buffered-comparison-report.md` and
`docs/benchmarks/buffered-comparison-results.json`. The earlier comparison and
validation logs are retained separately. No public API addition, dependency,
package version or default-feature change is required.


## Watcher-compatible adapter

With both `buffered` and `watchers`, `process_all_with_watchers` also selects the
experimental transform engine. Its signature, `WatcherList` type and report
format remain unchanged. Existing callbacks receive complete `Arc<str>` values
for before/current/next snapshots; an `after` snapshot is the output of the next
instruction, not an alias for the current state. At the last instruction it is
`None`. Observer callbacks are invoked in the same order relative to pipeline
execution as the original method, and the list resets only after successful JSON
export. Existing behavior for halt and failures is retained for comparison.

This compatibility adapter materializes each transformed document to construct
those snapshots. A tempfile-backed transform does not make arbitrary callbacks
streaming or impose a bounded memory limit. Caller input, multiple observer
snapshots and the final returned `String` still occupy memory. Benchmarks, rather
than the presence of a tempfile, should determine whether this path is useful.

```sh
cargo build --features buffered,watchers
python3 scripts/benchmark_buffered_comparison.py --include-watchers
```

The extended benchmark compiles the original with `watchers` and the candidate
with `buffered,watchers`, and reruns all four methods on identical fixtures. The
watcher list is defined in `examples/comparison_probe.rs`: bytes, Unicode scalar
count, words, lines, byte delta, unchanged, next-state byte length and instruction
label. All eight run at every instruction. Registration occurs once per process;
list reset/reuse and JSON export are included in measurements. The experiment
checks parsed JSON equality rather than raw file bytes because map key order is
not stable. Seven numeric/boolean fields are independently calculated from the
known intermediate fixture states; instruction labels match the original.

Reports: `docs/benchmarks/watchers-comparison-report.md` and
`docs/benchmarks/watchers-comparison-results.json`. Representative original and
candidate watcher JSON exports are retained in `docs/benchmarks/watchers-reports/`.
Additional untimed cases exercise empty/no-op, Unicode, variable resolution,
blocks, halt, fallback and failure behavior across list reuse.
