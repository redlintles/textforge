# Safe whole-document file I/O

## Result summary

read-copy, file, 1 worker(s): wall +1.4%, CPU +1.8%, peak RSS -43.4%.
read-copy, batch, 1 worker(s): wall +2.7%, CPU +7.0%, peak RSS -43.1%.
read-copy, batch, 4 worker(s): wall +2.6%, CPU +7.7%, peak RSS -48.2%.
mixed-64, file, 1 worker(s): wall -8.6%, CPU -8.1%, peak RSS -15.9%.
mixed-64, batch, 1 worker(s): wall -0.4%, CPU -0.4%, peak RSS -18.6%.
mixed-64, batch, 4 worker(s): wall +0.6%, CPU -0.3%, peak RSS -16.9%.

Before uses the existing mmap paths. After removes both mappings and the memmap2 dependency, reads through File::read_to_end, validates UTF-8 without a second byte copy and transfers the owned input into the non-buffered engine. Pipeline preparation and all earlier optimizations are identical in both snapshots.

## Measurements

| Workload | Method | Workers | Version | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |
|---|---|---:|---|---:|---:|---:|---|
| read-copy | file | 1 | before | 0.74 | 0.57 | 36.7 | 0.71–0.77 |
| read-copy | file | 1 | after | 0.75 | 0.58 | 20.8 | 0.74–0.82 |
| read-copy | batch | 1 | before | 0.74 | 0.57 | 36.7 | 0.72–0.76 |
| read-copy | batch | 1 | after | 0.76 | 0.61 | 20.9 | 0.73–0.82 |
| read-copy | batch | 4 | before | 0.78 | 0.91 | 132.8 | 0.41–0.80 |
| read-copy | batch | 4 | after | 0.80 | 0.98 | 68.8 | 0.44–1.03 |
| mixed-64 | file | 1 | before | 6.51 | 6.41 | 101.9 | 6.44–6.56 |
| mixed-64 | file | 1 | after | 5.95 | 5.89 | 85.7 | 5.93–6.01 |
| mixed-64 | batch | 1 | before | 5.61 | 5.56 | 85.6 | 5.56–5.67 |
| mixed-64 | batch | 1 | after | 5.59 | 5.54 | 69.7 | 5.53–5.60 |
| mixed-64 | batch | 4 | before | 1.64 | 6.27 | 357.2 | 1.59–1.66 |
| mixed-64 | batch | 4 | after | 1.65 | 6.25 | 296.8 | 1.62–2.01 |

## Methodology

5 fresh-process samples per version/configuration (60 samples), randomized paired order with fixed seed and one excluded paired warm-up. Same release worker, default features, locked offline dependencies, rustc 1.99.1 (58cf8f9c3 2026-10-13). Four distinct files contain identical Unicode records, each 16777281 bytes. read-copy runs null through eight batches or eight sequential groups of four files (512 MiB read/written); mixed-64 runs one group, 64 instructions per document: prefix, suffix, regex replacement and Unicode uppercase. File mode is sequential; batch uses a local Rayon pool created once per worker and reused across iterations. Both versions use the same pool.install + process_batch call sequence; process_batch_in_pool provides this sequence as a convenience and is separately tested from inside an executing instruction.

GNU time records wall time, user+system CPU time and peak RSS. Startup, pool creation, pipeline registration, reads, transforms and writes are included; compilation, Python reference generation and SHA-256 checks are excluded. Cache is warm, no fsync is requested, and timer resolution is 10 ms. RSS excludes unmapped filesystem page cache; resident file mappings are reclaimable and their removal does not imply the same reduction in system-wide physical memory. These workloads do not establish cold-disk throughput or statistical significance. Every output file from every timed/warm-up run matches an independent reference.

The buffered batch already did not use mmap, so it is not relabeled as a removal gain. process_file with buffered enabled still uses its existing buffered process_all dispatch after a safe read. The explicit process_file_buffered API remains the path for initial spooling. Complete-document materialization and transformation expansion remain possible; a pool bounds active tasks, not bytes.

## Validation and commit scope

See validation-results.json and validation/*.log.gz for the complete AGENTS.md checklist. Known repository-wide format/Clippy issues and private-parser doctest failures are reported rather than suppressed. Focused I/O tests cover Unicode, combining characters, emoji, NUL/CRLF, empty/large files, invalid UTF-8, missing files, invalid output paths, same-path replacement, partial batch failures, ordering, document isolation and pool reuse. A custom instruction verifies the actual worker executes in the supplied pool.

The new commits isolate the safe-read change, dependency removal, pool API and tests from prior uncommitted optimization work. Benchmark source archives include that earlier work equally in before/after; they are the precise measured versions, not a claim that all existing worktree changes belong to these commits.

## Reproduce

`python3 scripts/benchmark_safe_file_io.py`

Inputs are generated deterministically in data/safe-file-io; pipelines are in pipelines/safe-file-io. Source archives, worker/source fingerprints and raw samples are retained beside this report. A new candidate archive is taken on each run, while before-source.tar.gz is preserved.

## Final checklist results

| Scope | Command | Exit |
|---|---|---:|
| pre-implementation-commit | cargo fmt --check | 1 |
| pre-implementation-commit | cargo clippy --all-targets -- -D warnings | 101 |
| pre-implementation-commit | cargo clippy --all-targets --features test_access -- -D warnings | 101 |
| pre-implementation-commit | cargo test | 101 |
| pre-implementation-commit | cargo test --features bytecode | 101 |
| pre-implementation-commit | cargo test --features watchers | 101 |
| pre-implementation-commit | cargo test --features test_access | 0 |
| indexed-implementation | cargo fmt --check | 1 |
| indexed-implementation | cargo clippy --all-targets -- -D warnings | 101 |
| indexed-implementation | cargo clippy --all-targets --features test_access -- -D warnings | 101 |
| indexed-implementation | cargo test | 101 |
| indexed-implementation | cargo test --features bytecode | 101 |
| indexed-implementation | cargo test --features watchers | 101 |
| indexed-implementation | cargo test --features test_access | 0 |
| pre-benchmark-commit | cargo fmt --check | 1 |
| pre-benchmark-commit | cargo clippy --all-targets -- -D warnings | 101 |
| pre-benchmark-commit | cargo clippy --all-targets --features test_access -- -D warnings | 101 |
| pre-benchmark-commit | cargo test | 101 |
| pre-benchmark-commit | cargo test --features bytecode | 101 |
| pre-benchmark-commit | cargo test --features watchers | 101 |
| pre-benchmark-commit | cargo test --features test_access | 0 |

A later validation cache conflict was traced to reusing one target directory for the indexed snapshot and working tree; package artifacts were cleaned and the full checklist was repeated. Its diagnostic log is preserved separately. Benchmark builds each compiled their own library from a fresh workspace before copying the executable; future reproductions use separate per-version target directories.

The complete test_access suite passes in the final implementation and indexed commit. Other test configurations fail only the two inherited doctests that import a private parser. Format/Clippy failures predate this change; no lint allowances were added. Earlier development logs are preserved; the initial pool-observer fixture was corrected before the final checklist runs.
