# Batch input: mmap versus safe read

Both snapshots use the current prepared non-buffered engine (default features). The only production-code difference is the batch input block: mmap + UTF-8 validation + copy versus Read::read_to_end + String::from_utf8. The Vec allocation is transferred into String without another byte copy. process_file still uses mmap in both snapshots. The working implementation and dependencies are unchanged; archives and the exact replacement are retained here.

Four distinct 16 MiB Unicode files, same contents. read-copy: null pipeline, 8 batch calls (512 MiB read and written per worker). mixed-64: one batch, 64 instructions per file (prefix, suffix, regex replacement, Unicode uppercase). Runs use either one or four Rayon threads. Five fresh-process measurements per variant/case/thread count, randomized paired order, one excluded paired warm-up. Release, offline locked dependencies; Rust rustc 1.99.1 (58cf8f9c3 2026-10-13). GNU time measures startup, pipeline registration, reads, processing and writes. CPU is user+system; peak RSS excludes page cache. Cache is warm, files are not fsynced, and this is not a cold-storage throughput test. Timer resolution is 10 ms. Output hashes match independent Python references for every sample; empty files and invalid UTF-8 are checked in both builds.

| Pipeline | Threads | Variant | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |
|---|---:|---|---:|---:|---:|---|
| read-copy | 1 | mmap | 0.74 | 0.57 | 36.6 | 0.73–0.76 |
| read-copy | 1 | safe-read | 0.74 | 0.59 | 20.8 | 0.74–0.76 |
| read-copy | 4 | mmap | 0.43 | 0.96 | 132.6 | 0.42–0.45 |
| read-copy | 4 | safe-read | 0.45 | 1.01 | 68.8 | 0.44–0.46 |
| mixed-64 | 1 | mmap | 5.78 | 5.72 | 85.4 | 5.76–5.84 |
| mixed-64 | 1 | safe-read | 5.70 | 5.65 | 69.6 | 5.70–5.77 |
| mixed-64 | 4 | mmap | 1.68 | 6.52 | 362.2 | 1.68–1.74 |
| mixed-64 | 4 | safe-read | 1.68 | 6.40 | 292.0 | 1.64–1.69 |

## Interpretation

Removing mmap does not make whole-document transforms streaming and is not a global RAM limit. Control active tasks first (a local Rayon pool, or bounded submission), then consider a byte-budget scheduler using conservative per-pipeline expansion estimates. A single task may retain input/output/intermediate text; regex, JSON, reversal and arbitrary instructions cannot generally be split into independent chunks without changing semantics. TextBuffer can spill compatible operations, but fallbacks and String-returning methods materialize full text. An explicit file-to-file streaming contract would be needed for a compatible subset of transforms.

RSS of file-backed mappings is charged per process while resident, even though it is reclaimable page-cache-backed memory. Anonymous String allocations are different; lower RSS here does not mean identical reduction in total OS physical memory. mmap remains live throughout run_transform in the baseline; it also copies the complete input before execution. These results do not show an advantage for a zero-copy mmap design, which this engine does not implement.

## Reproduce

`python3 scripts/benchmark_batch_io.py`

Raw measurements, source snapshots, build logs and edge checks are stored beside this report. No dependency or public API changes were made.
