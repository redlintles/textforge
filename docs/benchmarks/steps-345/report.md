# Steps 3–5 performance comparison

## Decision summary

large-64: wall 5.00 → 2.80 s (-44.0%), CPU 4.99 → 2.79 s, peak RSS 134.4 → 118.3 MiB.

large-256: wall 49.50 → 26.32 s (-46.8%), CPU 49.47 → 26.29 s, peak RSS 168.1 → 167.2 MiB.

large-256-no-watchers: wall 18.97 → 12.17 s (-35.8%), CPU 18.96 → 12.16 s, peak RSS 102.9 → 102.3 MiB.

The constant-regex control is 77.8% faster, with 0.44 MiB more peak RSS from retained prepared metadata. The shared-statistics control is 17.8% faster with effectively unchanged RSS. These controls support the selected optimizations, while the full pipelines show the combined result. Compiled patterns remain resident, so complex-pattern pipelines can have a larger preparation memory cost than this control.

For the tested large mixed pipelines, these changes offer a substantial time/CPU reduction. RAM improves in the 16 MiB case and remains close in the 32 MiB cases. This does not establish superiority for every instruction mix or against the separately updated non-buffered engine; the comparison is against the saved alternative after steps 1–2.

The same 16/32 MiB files and 64/256-instruction mixed pipelines from the preceding experiment are reused. Before already contains the reliability/scheduling fixes (steps 1–2). After additionally prepares constant bindings, shares recognized default-watcher counts and retains text already required in RAM across compatible operations. All report bytes and transformed outputs are checked, including independent per-step metric references. Results describe the combined package; isolated controls below demonstrate constant-binding and shared-scan behavior without assigning the entire end-to-end gain to one change.

## Measurements

| Workload | Version | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |
|---|---|---:|---:|---:|---|
| large-64 | before | 5.000 | 4.990 | 134.4 | 4.95–5.03 |
| large-64 | after | 2.800 | 2.790 | 118.3 | 2.78–2.82 |
| large-256 | before | 49.500 | 49.470 | 168.1 | 49.19–49.64 |
| large-256 | after | 26.320 | 26.290 | 167.2 | 26.16–26.39 |
| large-256-no-watchers | before | 18.970 | 18.960 | 102.9 | 18.63–19.08 |
| large-256-no-watchers | after | 12.170 | 12.160 | 102.3 | 11.99–12.28 |
| prepared-constants | before | 0.360 | 0.360 | 5.2 | 0.36–0.38 |
| prepared-constants | after | 0.080 | 0.070 | 5.7 | 0.08–0.08 |
| shared-statistics | before | 3.140 | 3.130 | 67.1 | 3.10–4.72 |
| shared-statistics | after | 2.580 | 2.570 | 67.1 | 2.53–2.62 |

| Workload | Wall change | CPU change | Peak RSS change |
|---|---:|---:|---:|
| large-64 | -44.0% | -44.1% | -12.0% |
| large-256 | -46.8% | -46.9% | -0.5% |
| large-256-no-watchers | -35.8% | -35.9% | -0.5% |
| prepared-constants | -77.8% | -80.6% | +8.3% |
| shared-statistics | -17.8% | -17.9% | -0.1% |

## Methodology and controls

5 fresh-process samples per case/version after one excluded paired warm-up. Pair/version order is randomized with a fixed seed. Identical release builds use buffered,watchers, the same locked dependencies and worker. GNU time captures wall, user+system CPU and peak RSS (10 ms time resolution). Input reads, registration/preparation, execution, JSON serialization/export, reset and output writes are timed. Checks and builds are outside the timed worker. RSS excludes filesystem page cache; warm-cache timings are not cold-disk guarantees. No statistical-significance claim is made.

large-64: 16 MiB / 64 instructions, one run with eight watchers. large-256: 32 MiB / 256 instructions, one run with the same watchers. Both repeat prefix, suffix, regex replacement and Unicode uppercase. large-256-no-watchers: the same large pipeline using process_all, showing the effect outside reports. Watchers: bytes, Unicode scalar count, words, lines, byte delta, unchanged, next-state bytes and instruction label.

prepared-constants: 128 identical literal-regex instructions, 500 runs on ~1 KiB of Unicode text. Registration is included, so front-loaded compilation is counted. shared-statistics: 64 direct watcher steps on one immutable 32 MiB snapshot using the same eight functions; no transforms or temporary-buffer transitions. This isolates default-statistics computation, not large-file pipeline execution.

Every final output matches an independent SHA-256 reference. Every watcher row matches independent numeric/boolean references and the entire JSON, including instruction labels, matches between before/after. Sorted JSON is deterministic across fresh processes. Compressed example exports, exact source archives, worker hash, source hashes and raw samples are retained in this directory.

## Compatibility and resource policy

Only registered private pipeline instances cache bindings. Exported TokenWrapper clones discard the cache, preventing stale public-template mutations. Custom instructions must opt in via the new default-false supports_preparation method; stateless built-in transforms opt in. Variable references, all interpolation delimiters and nested-token parameters stay dynamic. Invalid bindings retain execution-time errors and unreachable invalid instructions do not fail registration.

Shared statistics recognize exact function-item TypeIds for current_len_chars, word_count, line_count, char_delta and word_count_delta. No function-pointer address comparison is used. Aliases of the same function work; function pointers and wrapping/custom closures stay on the ordinary callback path. No text is retained by the statistics cache beyond the current call, and custom replacement callbacks clear the recognition metadata.

Public TextBuffer constructors/file APIs retain their explicit spool placement behavior. String-returning processor adapters may retain already materialized fallback results; watcher adapters borrow existing Arc snapshots rather than spooling and rereading them. Explicit spill remains available and error-safe. This is a per-path performance policy, not a process-wide RAM bound. Arbitrary callbacks can still retain complete snapshots; Unicode regex and uppercase semantics remain whole-document.

## Reproduction

`python3 scripts/benchmark_steps345.py`

`--report-only` regenerates this report. Linux/GNU time, Python 3.12+, Rust and cached Cargo dependencies are required. Fixture data reuse data/watcher-reliability/ and add data/steps-345/; pipelines are in pipelines/steps-345/. No dependency, version, bytecode or file-extension change is introduced.

## Validation

| Command | Exit |
|---|---:|
| cargo test --offline --features test_access | 0 |
| cargo clippy --offline --all-targets --features test_access -- -D warnings | 101 |
| cargo fmt --check | 1 |
| cargo clippy --offline --all-targets -- -D warnings | 101 |
| cargo test --offline | 101 |
| cargo test --offline --features bytecode | 101 |
| cargo test --offline --features watchers | 101 |
| cargo test --offline --features buffered,watchers | 101 |

Full test_access passes, including preparation, dynamic context, cache invalidation, Unicode statistics, retained-buffer transitions, safe allocation refusal and report-reliability tests. Other full test configurations retain known private-parser doctest failures; repository-wide format/Clippy retain inherited issues. Logs are compressed in validation/.
