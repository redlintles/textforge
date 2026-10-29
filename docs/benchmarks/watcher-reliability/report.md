# Watcher reliability and scheduling benchmark

## Decision summary

With 16 MiB and 64 instructions, wall time is 4.68 → 4.84 s, CPU 4.68 → 4.84 s and peak RSS 150.7 → 134.6 MiB. Each report now has exactly 64 complete rows rather than 512 rows padded with empty objects.

With 32 MiB and 256 instructions, wall time is 46.03 → 48.16 s, CPU 46.00 → 48.14 s and peak RSS 168.1 → 168.1 MiB. Each report now has exactly 256 complete rows rather than 2048 rows padded with empty objects.

The fixes improve report integrity and remove avoidable collection allocations. They do not reduce full-document snapshot construction or the repeated scans performed by character/word/line watchers, so large-file end-to-end gains can be much smaller than collection-only gains. Small percentage differences inside the observed timing ranges are inconclusive; no statistical-significance claim is made.

The end-to-end runs show no speedup: median wall time increases 3.4% for 64 instructions and 4.6% for 256 instructions. The collection-growth control is 38.1% faster and uses 43.7% less peak RSS; the reuse control is 20.9% faster. These collection-only gains must not be presented as large-file pipeline speedups.

Keep these changes for complete/deterministic reports and lower observer bookkeeping cost. They are not sufficient as a large-file throughput optimization. Further changes to snapshot materialization or repeated metric scans need a separate experiment. The before/after comparison measures the combined package of reliability and scheduling changes, not each change in isolation.

Both versions use the same experimental buffered engine, with buffered,watchers features, locked dependencies and release optimization. Before is the saved working-source snapshot immediately before these fixes; after changes WatcherList collection, scheduling, bounded reuse and JSON publication. No shared-statistics, regex or transform optimization is included.

Five fresh-process samples per case by default; actual repetitions: 5. One paired warm-up per case is excluded. Version order is randomized within each pair. CPU = user + system; RSS is peak resident process memory, excluding filesystem page cache. GNU time has 10 ms resolution. Builds and validation finish before measurement. Each long-pipeline sample executes once; collection controls are described separately.

| Workload | Version | Wall median (s) | CPU median (s) | Peak RSS median (MiB) | Report bytes | Report rows | Wall range (s) |
|---|---|---:|---:|---:|---:|---:|---|
| large-64 | before | 4.680 | 4.680 | 150.7 | 16677 | 512 | 4.61–4.84 |
| large-64 | after | 4.840 | 4.840 | 134.6 | 13989 | 64 | 4.80–4.95 |
| large-256 | before | 46.030 | 46.000 | 168.1 | 66693 | 2048 | 45.89–46.60 |
| large-256 | after | 48.160 | 48.140 | 168.1 | 55941 | 256 | 47.58–48.19 |
| collection-growth | before | 0.630 | 0.620 | 42.2 | 4820002 | 160000 | 0.60–0.69 |
| collection-growth | after | 0.390 | 0.390 | 23.8 | 3980002 | 20000 | 0.38–0.42 |
| collection-reuse | before | 1.100 | 1.090 | 4.5 | 61698 | 2048 | 1.04–1.11 |
| collection-reuse | after | 0.870 | 0.850 | 4.5 | 50946 | 256 | 0.82–0.90 |

## Relative changes (after versus before)

| Workload | Wall | CPU | Peak RSS | Report bytes |
|---|---:|---:|---:|---:|
| large-64 | +3.4% | +3.4% | -10.7% | -16.1% |
| large-256 | +4.6% | +4.7% | -0.0% | -16.1% |
| collection-growth | -38.1% | -37.1% | -43.7% | -17.4% |
| collection-reuse | -20.9% | -22.0% | +0.0% | -17.4% |

Collection controls were rerun separately with longer fixed iteration counts after all large-file samples finished, to reduce timer quantization. The same source snapshots and worker are verified by hashes before that rerun.

## Workloads and correctness

large-64: 16 MiB, 64 instructions. large-256: 32 MiB, 256 instructions. Both repeat prefix, suffix, regex replacement and Unicode uppercase, including native and fallback instructions. Growth is bounded: the replacement happens once per original record, and each four-instruction group adds only two delimiter bytes. Eight watchers are identical across versions: bytes, characters, words, lines, byte delta, unchanged, next bytes and instruction. Independent references check every numeric/boolean metric at every step. Full meaningful report rows, including instruction labels, must match across versions. The comparison removes only the known empty legacy trailing rows; the corrected exporter must emit no such rows.

collection-growth: a fixed ~1 KiB shared context, 20,000 direct watcher steps per run, 10 exports/resets. collection-reuse: the same context, 256 steps per run, 2,000 exports/resets using the same list. These controls isolate scheduling, row allocation and report export; they are not measurements of processing a large file. Eight cheap metadata callbacks avoid full-text scans. Output-file writes and JSON export remain timed.

All transformed outputs match independent SHA-256 references. Original and new reports are checked after every process; candidate raw JSON bytes must be identical across fresh processes. Sample reports are retained as gzip files, alongside source snapshots and raw samples. The new report keeps the JSON array/string-value schema but removes extra empty rows and sorts keys; callbacks now execute in watcher-name order. Replacement callbacks and reset behavior are covered by regressions.

## Reliability guarantees and limits

The exporter serializes through a checked buffered writer into a unique same-directory temporary file, flushes it and renames it over the destination only on success. Failed writes preserve the old report and cleanup the temporary file; regression tests inject short writes, write-zero, write/flush failures and concurrent publishers. Publishing is atomic on the tested Linux filesystem. This does not promise persistence across power loss, preserve old inode permissions, or recover from process abort/OOM. Reuse retains at most 256 empty row maps with capacity at most 128 entries; values and oversized storage are released. No new dependency or public signature was added.

## Reproduction

`python3 scripts/benchmark_watcher_reliability.py`

Requires Linux, GNU time, Python 3.12+, Rust and cached locked Cargo dependencies. Data: data/watcher-reliability/. Pipelines: pipelines/watcher-reliability/. Before source: baseline-source.tar.gz. After source: candidate-source.tar.gz. Raw results: results.json. `--report-only` regenerates this report without rerunning measurements. Validation: validation-results.json and validation/*.log.gz.

## Validation

| Command | Exit |
|---|---:|
| cargo test --offline --features test_access | 0 |
| cargo test --offline --features watchers | 101 |
| cargo clippy --offline --all-targets --features test_access -- -D warnings | 101 |
| cargo fmt --check | 1 |

Full test_access passes, including all reliability regressions and doctests. The standalone watchers configuration retains the two known private-parser doctest failures; its unit/integration tests pass. Repository formatting and strict Clippy retain inherited diagnostics. No new Clippy diagnostics target the optimized watcher module or tests.
