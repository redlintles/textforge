# Four-method benchmark including watchers — Rust 1.99.1

## Decision summary

The extended comparison contains 80 measured fresh-process samples across four methods, two fixtures and two implementations, with five samples per case after warm-up. The 1 MiB sample runs 100 calls and the 8 MiB sample runs four calls; batch processes four documents per call. All outputs match independent SHA-256 references. All measured watcher reports match the original parsed JSON; nine additional watcher equivalence cases and 18 debug equivalence checks pass.

For the 8 MiB normalization workload, watcher wall time changes from 0.51 to 0.77 s (+51.0%), CPU time from 0.51 to 0.75 s (+47.1%), and peak RSS from 61.3 to 53.3 MiB (-13.2%). For the 1 MiB prefix/suffix workload, watcher wall time changes from 0.47 to 0.86 s (+83.0%), CPU time from 0.47 to 0.85 s (+80.9%), and peak RSS from 10.8 to 10.0 MiB (-7.6%).

The watcher adapter is a compatibility path, not a streaming observer API. Existing callbacks require complete Arc<str> snapshots of before/current/next states, so spooling transformations still requires materialization. The extra temporary-file I/O is consistent with the higher CPU and wall times. Process RSS excludes filesystem page-cache memory, so the RSS reduction is not a measurement of total host-memory savings.

For these workloads and the fixed 1 MiB threshold, retain the original watcher implementation when processing time or CPU efficiency is the priority. The experimental watcher path offers a modest RSS reduction at a substantial time cost. The larger memory benefits remain in debug and batch; ordinary process_all does not show a useful RSS reduction. Keep buffered opt-in and avoid switching all methods by default. These observations do not establish behavior for larger documents, other thresholds or cold caches.

Identical release workers; original enables watchers and candidate enables buffered,watchers; per-buffer and debug-log spill thresholds are 1 MiB, using the system temporary directory; locked dependencies; one excluded warm-up per case; randomized paired revision order; four Rayon threads. Five fresh-process samples by default. Each sample includes startup, pipeline parsing, input reads, repeated executions and output writes (100 wrap / 4 normalize executions per worker with default options; exact document counts are in the raw samples). Batch executes four documents per call. Debug stdout goes to /dev/null, so formatting/allocation remain measured but terminal rendering does not.

CPU = user + system time; CPU% may exceed 100 with parallel execution. RSS is the maximum resident memory of the entire worker, not allocated bytes. GNU time has 10 ms time resolution; small timings are noisy. CPU percentages omit samples whose wall time rounds to zero; nan means no measurable wall time. No claim of statistical significance is made.

The same four API signatures are exercised in both versions. With buffered enabled, process_all uses TextBuffer and materializes its String result; process_batch keeps Rayon parallelism and streams each output file; process_all_with_debug spools the complete log before printing it. Non-native transforms still materialize the document. Debug with forced color also materializes each snapshot to preserve embedded ANSI resets. Disk page-cache memory is not included in process RSS. These are end-to-end warm-cache measurements, not isolated transform timings.

| Scenario | Method | Version | Wall median (s) | CPU median (s) | CPU% median | RSS median (MiB) | Wall range (s) |
|---|---|---|---:|---:|---:|---:|---:|
| normalize | process_all | original | 0.230 | 0.230 | 100.0 | 28.9 | 0.23–0.25 |
| normalize | process_all | candidate | 0.370 | 0.360 | 97.3 | 29.2 | 0.36–0.38 |
| normalize | process_all_with_debug | original | 0.690 | 0.680 | 98.6 | 126.1 | 0.68–0.73 |
| normalize | process_all_with_debug | candidate | 0.780 | 0.770 | 98.7 | 29.2 | 0.77–0.80 |
| normalize | process_all_with_watchers | original | 0.510 | 0.510 | 98.1 | 61.3 | 0.50–0.55 |
| normalize | process_all_with_watchers | candidate | 0.770 | 0.750 | 98.6 | 53.3 | 0.74–0.81 |
| normalize | process_batch | original | 0.310 | 1.000 | 333.3 | 133.4 | 0.29–0.33 |
| normalize | process_batch | candidate | 0.430 | 1.570 | 357.1 | 70.2 | 0.42–0.50 |
| wrap | process_all | original | 0.200 | 0.200 | 100.0 | 6.7 | 0.20–0.24 |
| wrap | process_all | candidate | 0.350 | 0.340 | 97.1 | 6.9 | 0.33–0.40 |
| wrap | process_all_with_debug | original | 0.670 | 0.660 | 98.5 | 16.6 | 0.64–0.68 |
| wrap | process_all_with_debug | candidate | 0.910 | 0.900 | 100.0 | 6.9 | 0.89–0.92 |
| wrap | process_all_with_watchers | original | 0.470 | 0.470 | 100.0 | 10.8 | 0.45–0.49 |
| wrap | process_all_with_watchers | candidate | 0.860 | 0.850 | 98.9 | 10.0 | 0.83–0.92 |
| wrap | process_batch | original | 0.240 | 0.750 | 340.9 | 16.6 | 0.22–0.24 |
| wrap | process_batch | candidate | 0.390 | 1.440 | 385.0 | 9.4 | 0.36–0.42 |

Every measured output matched an independently computed SHA-256 reference, including every batch output. Input data contain accents, combining marks, emoji and ZWJ sequences.

## Reproduction

`python3 scripts/benchmark_buffered_comparison.py --include-watchers`

Requires Linux, GNU time, Python 3.12+, Rust and cached Cargo dependencies. Input fixtures and textual pipelines live in the repository root under data/benchmark-comparison and pipelines/benchmark-comparison. Generated binaries and output files are temporary. Raw samples, machine metadata, exact revisions and fixture hashes: watchers-comparison-results.json.

## Compiler regression scope

Only Rust 1.99.1 is installed. Tests on this compiler can reveal failures, but cannot establish that an issue was introduced by the upgrade without running the same commit on the previous compiler. See watcher-validation-results.json for validation of the current implementation.

## Candidate relative to original

| Scenario | Method | Wall change | CPU-time change | Peak-RSS change |
|---|---|---:|---:|---:|
| normalize | process_all | +60.9% | +56.5% | +0.8% |
| normalize | process_all_with_debug | +13.0% | +13.2% | -76.9% |
| normalize | process_all_with_watchers | +51.0% | +47.1% | -13.2% |
| normalize | process_batch | +38.7% | +57.0% | -47.4% |
| wrap | process_all | +75.0% | +70.0% | +4.0% |
| wrap | process_all_with_debug | +35.8% | +36.4% | -58.3% |
| wrap | process_all_with_watchers | +83.0% | +80.9% | -7.6% |
| wrap | process_batch | +62.5% | +92.0% | -43.5% |

Negative changes mean lower resource use or faster processing. Five samples do not establish statistical significance. Compare the wall ranges and consider CPU, RSS and extra temporary-file I/O together. Each input and output uses the same bytes in both implementations. The debug equivalence suite compares full stdout and results with and without forced colors, including empty input, Unicode, embedded ANSI resets, variables, blocks, halt, fallback transforms and a spilled document; transformation failure must not print partial logs. Detailed check results and source fingerprints are in the raw JSON.

Enable the adapters using cargo build --features buffered. Without that feature, the original methods remain selected. No new dependency or public signature is introduced. The earlier comparison report is retained separately.

## Interpretation

The adapters trade lower batch/debug process RSS for more CPU time and temporary-file I/O. For these workloads, process_all offers no meaningful RSS improvement and takes longer. The caller-owned input and final String allocation remain, and normalization uses full-document fallbacks. Batch and debug reduce retained memory, but the three previously measured methods have higher median wall time with the candidate. This result supports keeping the feature opt-in; it does not support switching the default implementation for these loads. The threshold is fixed at 1 MiB for this experiment; conclusions do not cover other thresholds, larger workloads or cold disk caches.

## Validation of the new implementation

| Command | Result |
|---|---|
| cargo test --offline --features test_access | PASS |
| cargo clippy --offline --all-targets --features test_access -- -D warnings | FAIL |
| cargo fmt --check | FAIL |
| cargo test --offline --features watchers | FAIL |
| cargo test --offline --features buffered,watchers | FAIL |
| cargo test --offline --features buffered | FAIL |

Full test_access passes, including new watcher snapshot, reset/reuse, Unicode and error tests. All library/integration tests in the watchers, buffered+watchers and buffered-only configurations pass; the full cargo test commands retain the two known private-parser doctest failures. Repository formatting and strict Clippy retain inherited failures; no diagnostics point to the new adapter module or tests. Full logs and summaries are in watcher-validation/ and watcher-validation-results.json.

## Watcher workload and compatibility

Eight watchers run after every instruction: UTF-8 bytes, Unicode scalar count, words, lines, byte delta, unchanged, next-state byte length and instruction label. They exercise all four WatcherContext fields, using six existing default functions and two small custom functions. Registration happens once per worker, and the same list is reused across calls; snapshot construction, scanning, JSON serialization/export and automatic reset are timed. Report equality is checked after each process, outside the timed worker. Canonical JSON comparison ignores HashMap key ordering but retains every array entry, including existing empty report entries.

The current WatcherContext requires complete Arc<str> snapshots. The adapter therefore materializes before/current/next states while transformations use TextBuffer; it cannot guarantee bounded memory. It preserves existing lookahead behavior (after is the next instruction output; None at EOF), error propagation and reset-after-success behavior. Existing watcher semantics, including how end is handled, are preserved for this comparison.

Detailed sample reports are saved in watchers-reports/. Each report is compared with the original implementation and all seven numeric/boolean fields are independently checked against Python-computed intermediate states for both measured fixtures. Additional untimed equivalence cases cover empty/no-op, Unicode, variables, blocks, halt, fallback and failures. Source and worker hashes are retained in the raw results.

Watcher validation for this run is recorded in watcher-validation-results.json and watcher-validation/. The earlier buffered-validation files describe the preceding three-method experiment.

Watchers, wrap: wall 0.470 → 0.860 s; CPU 0.470 → 0.850 s; peak RSS 10.8 → 10.0 MiB.

Watchers, normalize: wall 0.510 → 0.770 s; CPU 0.510 → 0.750 s; peak RSS 61.3 → 53.3 MiB.
