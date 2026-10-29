# Revision benchmark — Rust 1.99.1

Identical release workers; original uses default features and candidate enables buffered; per-buffer and debug-log spill thresholds are 1 MiB, using the system temporary directory; locked dependencies; one excluded warm-up per case; randomized paired revision order; four Rayon threads. Five fresh-process samples by default. Each sample includes startup, pipeline parsing, input reads, repeated executions and output writes (100 wrap / 4 normalize executions per worker with default options; exact document counts are in the raw samples). Batch executes four documents per call. Debug stdout goes to /dev/null, so formatting/allocation remain measured but terminal rendering does not.

CPU = user + system time; CPU% may exceed 100 with parallel execution. RSS is the maximum resident memory of the entire worker, not allocated bytes. GNU time has 10 ms time resolution; small timings are noisy. CPU percentages omit samples whose wall time rounds to zero; nan means no measurable wall time. No claim of statistical significance is made.

The same three API signatures are exercised in both versions. With buffered enabled, process_all uses TextBuffer and materializes its String result; process_batch keeps Rayon parallelism and streams each output file; process_all_with_debug spools the complete log before printing it. Non-native transforms still materialize the document. Debug with forced color also materializes each snapshot to preserve embedded ANSI resets. Disk page-cache memory is not included in process RSS. These are end-to-end warm-cache measurements, not isolated transform timings.

| Scenario | Method | Version | Wall median (s) | CPU median (s) | CPU% median | RSS median (MiB) | Wall range (s) |
|---|---|---|---:|---:|---:|---:|---:|
| normalize | process_all | original | 0.230 | 0.220 | 95.7 | 28.9 | 0.23–0.23 |
| normalize | process_all | candidate | 0.370 | 0.360 | 100.0 | 29.0 | 0.36–0.37 |
| normalize | process_all_with_debug | original | 0.690 | 0.670 | 98.5 | 126.0 | 0.68–0.71 |
| normalize | process_all_with_debug | candidate | 0.780 | 0.770 | 98.7 | 29.1 | 0.77–0.86 |
| normalize | process_batch | original | 0.290 | 1.000 | 337.9 | 133.4 | 0.29–0.35 |
| normalize | process_batch | candidate | 0.420 | 1.530 | 361.9 | 70.1 | 0.42–0.44 |
| wrap | process_all | original | 0.200 | 0.190 | 95.0 | 6.6 | 0.20–0.21 |
| wrap | process_all | candidate | 0.340 | 0.330 | 100.0 | 6.8 | 0.33–0.34 |
| wrap | process_all_with_debug | original | 0.660 | 0.650 | 98.5 | 16.6 | 0.65–0.68 |
| wrap | process_all_with_debug | candidate | 0.900 | 0.890 | 100.0 | 6.8 | 0.88–0.90 |
| wrap | process_batch | original | 0.220 | 0.740 | 331.8 | 16.7 | 0.22–0.27 |
| wrap | process_batch | candidate | 0.360 | 1.390 | 386.1 | 9.3 | 0.35–0.39 |

Every measured output matched an independently computed SHA-256 reference, including every batch output. Input data contain accents, combining marks, emoji and ZWJ sequences.

## Reproduction

`python3 scripts/benchmark_buffered_comparison.py`

Requires Linux, GNU time, Python 3.12+, Rust and cached Cargo dependencies. Input fixtures and textual pipelines live in the repository root under data/benchmark-comparison and pipelines/benchmark-comparison. Generated binaries and output files are temporary. Raw samples, machine metadata, exact revisions and fixture hashes: buffered-comparison-results.json.

## Compiler regression scope

Only Rust 1.99.1 is installed. Tests on this compiler can reveal failures, but cannot establish that an issue was introduced by the upgrade without running the same commit on the previous compiler. See validation-results.json for the feature/check matrix.

## Candidate relative to original

| Scenario | Method | Wall change | CPU-time change | Peak-RSS change |
|---|---|---:|---:|---:|
| normalize | process_all | +60.9% | +63.6% | +0.3% |
| normalize | process_all_with_debug | +13.0% | +14.9% | -76.9% |
| normalize | process_batch | +44.8% | +53.0% | -47.5% |
| wrap | process_all | +70.0% | +73.7% | +2.6% |
| wrap | process_all_with_debug | +36.4% | +36.9% | -59.0% |
| wrap | process_batch | +63.6% | +87.8% | -44.2% |

Negative changes mean lower resource use or faster processing. Five samples do not establish statistical significance. Compare the wall ranges and consider CPU, RSS and extra temporary-file I/O together. Each input and output uses the same bytes in both implementations. The debug equivalence suite compares full stdout and results with and without forced colors, including empty input, Unicode, embedded ANSI resets, variables, blocks, halt, fallback transforms and a spilled document; transformation failure must not print partial logs. Detailed check results and source fingerprints are in the raw JSON.

Enable the adapters using cargo build --features buffered. Without that feature, the original methods remain selected. No new dependency or public signature is introduced. The earlier comparison report is retained separately.

## Interpretation

The adapters trade lower batch/debug process RSS for more CPU time and temporary-file I/O. For these workloads, process_all offers no meaningful RSS improvement and takes longer. The caller-owned input and final String allocation remain, and normalization uses full-document fallbacks. Batch and debug reduce retained memory, but all six measured cases have higher median wall time with the candidate. This result supports keeping the feature opt-in; it does not support switching the default implementation for these loads. The threshold is fixed at 1 MiB for this experiment; conclusions do not cover other thresholds, larger workloads or cold disk caches.

## Validation of the new implementation

| Command | Result |
|---|---|
| cargo test --offline --features test_access | PASS |
| cargo clippy --offline --all-targets --features test_access -- -D warnings | FAIL |
| cargo fmt --check | FAIL |
| cargo test --offline | FAIL |
| cargo test --offline --features bytecode | FAIL |
| cargo test --offline --features watchers | FAIL |
| cargo test --offline --features buffered --lib --tests | PASS |

test_access passes, including the new adapters and resource-limit regression. Buffered library/integration tests pass. Default, bytecode and watchers retain the two known private-parser doctest failures. Repository-wide formatting and strict Clippy retain inherited failures; the new adapter module and tests have no Clippy diagnostics. Full logs and test summaries are retained in buffered-validation/ and buffered-validation-results.json. Earlier validation-results.json describes the preceding commit comparison.
