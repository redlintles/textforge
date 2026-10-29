# Reliable watcher reports and lower collection overhead

This change implements the first two agreed improvements only. It does not
change transform algorithms, combine statistics scans, cache regexes or redesign
WatcherContext. It applies to both original and experimental processing paths
because they share WatcherList.

## Collection and compatibility

Each step with scheduled watchers adds one complete result map. The previous
implementation appended one map per watcher and filled only the step-indexed
map, leaving trailing empty objects. The JSON remains an array of string-valued
objects, but those extra rows are removed. Consumers must no longer interpret
array length as watcher-count times instruction-count.

Scheduled callbacks are prepared at registration/scheduling time and executed in
lexicographic watcher-name order. Replacing a registered callback updates its
scheduled entry; scheduling the same name twice does not duplicate execution.
The per-step path no longer allocates a copied schedule, looks up each callback
in another map, or allocates an owned copy of each static watcher name. Existing
callback signatures, ExecutionWindows::All, reset/reuse and context semantics
remain available. Callbacks that depended on the old unspecified HashMap
iteration order now receive a defined order.

Reset retains at most 256 empty result maps with capacity for at most 128 entries
and releases unusually large outer vectors (capacity over 4096 rows). Result
strings are dropped, including large custom values. These bounds apply to
retained metadata buffers, not process RAM or callback-retained snapshots.

## JSON publication

Serialization streams into a checked 64 KiB buffered writer, sorting keys one
row at a time without copying result values or constructing a whole JSON String.
The destination is replaced with a same-directory rename only after complete
serialization and successful flush. Temporary names use the existing UUID
dependency and create_new prevents clobbering another temporary report.

Write/serialization/publication errors leave the old destination intact.
Unpublished temporary files are removed by a cleanup guard on errors or
unwinding (filesystem cleanup is best effort). A successful report export leaves
the list populated until the processor performs its existing reset; failed
exports leave results available for inspection or retry. Concurrent publishers
produce one complete winning report, rather than interleaved fragments.

Atomic replacement is tested on Linux. It does not guarantee power-loss
persistence, preserve existing inode permissions, or recover from abort/OOM.
No fsync is claimed or added. The API still accepts arbitrary callbacks that
return complete Strings and can retain Arc snapshots.

## Regression coverage

Tests cover one row per step, deterministic key order, replacement/rescheduling,
bounded metadata reuse, large-value release, complete short writes, write-zero,
write and flush failures, destination preservation, failed-rename cleanup,
concurrent publishers and counter overflow. Existing snapshot/Unicode/list-reuse
regressions and the full test_access suite remain passing.

## Benchmark

```sh
python3 scripts/benchmark_watcher_reliability.py
```

The saved pre-fix alternative is compared with the modified alternative using
identical release buffered,watchers builds. Native and fallback instructions are
mixed in pipelines with 16 MiB / 64 steps and 32 MiB / 256 steps. The same eight
watchers scan/count text and inspect before/current/next states in both versions.
Collection-only controls separately measure metadata growth and repeated reset.
Every meaningful row is compared and numeric/boolean metrics are independently
calculated from the fixture; transformed output uses SHA-256 references.

Source archives, raw samples, compressed sample reports, validation logs and a
summary are retained in docs/benchmarks/watcher-reliability/. Fixtures are in
data/watcher-reliability/ and pipelines/watcher-reliability/. `--report-only`
regenerates the report. `--controls-only` can repeat longer collection controls
while retaining large-case results, but refuses changed source/worker hashes.
