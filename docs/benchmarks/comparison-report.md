# Revision benchmark — Rust 1.99.1

Identical release workers; default features; locked dependencies; one excluded warm-up per case; randomized paired revision order; four Rayon threads. Five fresh-process samples by default. Each sample includes startup, pipeline parsing, input reads, repeated executions and output writes (100 wrap / 4 normalize executions per worker with default options; exact document counts are in the raw samples). Batch executes four documents per call. Debug stdout goes to /dev/null, so formatting/allocation remain measured but terminal rendering does not.

CPU = user + system time; CPU% may exceed 100 with parallel execution. RSS is the maximum resident memory of the entire worker, not allocated bytes. GNU time has 10 ms time resolution; small timings are noisy. CPU percentages omit samples whose wall time rounds to zero; nan means no measurable wall time. No claim of statistical significance is made.

Both revisions use the requested existing APIs. The candidate’s opt-in buffered APIs are separate and are NOT exercised by these three methods; consult docs/tempfile-experiment.md for that distinct experiment.

| Scenario | Method | Version | Wall median (s) | CPU median (s) | CPU% median | RSS median (MiB) | Wall range (s) |
|---|---|---|---:|---:|---:|---:|---:|
| normalize | process_all | original | 0.230 | 0.220 | 95.7 | 29.0 | 0.23–0.23 |
| normalize | process_all | candidate | 0.220 | 0.220 | 96.0 | 29.0 | 0.22–0.25 |
| normalize | process_all_with_debug | original | 0.700 | 0.680 | 98.6 | 126.1 | 0.68–0.72 |
| normalize | process_all_with_debug | candidate | 0.690 | 0.680 | 98.5 | 126.2 | 0.68–0.74 |
| normalize | process_batch | original | 0.320 | 1.020 | 330.3 | 133.6 | 0.29–0.34 |
| normalize | process_batch | candidate | 0.300 | 0.980 | 330.0 | 133.7 | 0.29–0.32 |
| wrap | process_all | original | 0.200 | 0.200 | 100.0 | 6.6 | 0.19–0.26 |
| wrap | process_all | candidate | 0.200 | 0.200 | 100.0 | 6.7 | 0.20–0.21 |
| wrap | process_all_with_debug | original | 0.680 | 0.680 | 98.6 | 16.5 | 0.65–0.69 |
| wrap | process_all_with_debug | candidate | 0.670 | 0.670 | 98.6 | 16.8 | 0.65–0.72 |
| wrap | process_batch | original | 0.250 | 0.760 | 326.1 | 16.6 | 0.23–0.28 |
| wrap | process_batch | candidate | 0.250 | 0.790 | 335.5 | 16.7 | 0.23–0.31 |

Every measured output matched an independently computed SHA-256 reference, including every batch output. Input data contain accents, combining marks, emoji and ZWJ sequences.

## Reproduction

`python3 scripts/benchmark_comparison.py`

Requires Linux, GNU time, Python 3.12+, Rust and cached Cargo dependencies. Input fixtures and textual pipelines live in the repository root under data/benchmark-comparison and pipelines/benchmark-comparison. Generated binaries and output files are temporary. Raw samples, machine metadata, exact revisions and fixture hashes: comparison-results.json.

## Compiler regression scope

Only Rust 1.99.1 is installed. Tests on this compiler can reveal failures, but cannot establish that an issue was introduced by the upgrade without running the same commit on the previous compiler. See validation-results.json for the feature/check matrix.

## Observed comparison

Candidate changes relative to original (medians; negative means lower):

| Scenario | Method | Wall | CPU time | Peak RSS |
|---|---|---:|---:|---:|
| wrap | process_all | +0.0% | +0.0% | +1.5% |
| wrap | process_batch | +0.0% | +3.9% | +0.5% |
| wrap | process_all_with_debug | -1.5% | -1.5% | +1.5% |
| normalize | process_all | -4.3% | +0.0% | +0.2% |
| normalize | process_batch | -6.2% | -3.9% | +0.1% |
| normalize | process_all_with_debug | -1.4% | +0.0% | +0.1% |

The implementation bodies of process_all, process_batch and process_all_with_debug are unchanged between these commits. This comparison provides a baseline and output-equivalence check; it does not establish a speedup from the opt-in buffered implementation. The 1 MiB scenario uses 100 internal executions per sample to avoid zero-duration measurements; differences of only a few percent still require longer repeated sessions before drawing optimization conclusions.

## Rust 1.99.1 validation

| Check | Original | Candidate |
|---|---|---|
| cargo fmt --check | FAIL | FAIL |
| cargo clippy --all-targets -- -D warnings | FAIL | FAIL |
| cargo clippy --all-targets --features test_access -- -D warnings | FAIL | FAIL |
| cargo test | FAIL | FAIL |
| cargo test --features bytecode | FAIL | FAIL |
| cargo test --features watchers | FAIL | FAIL |
| cargo test --features test_access | PASS | PASS |

Both revisions fail formatting and strict Clippy checks. On both revisions, default, bytecode-only and watchers-only cargo test runs reach failing doctests that reference the private parser module. Full test_access runs pass. These findings occur in both revisions on 1.99.1; the available evidence does not identify a new candidate regression or prove a compiler-upgrade regression. No earlier compiler is installed for a controlled compiler comparison. Full outputs are retained under validation/.

The final measurements were collected after the validation matrix finished; earlier samples taken concurrently with validation were discarded. Builds finish before any warm-up or timed run. Other host activity is uncontrolled.
