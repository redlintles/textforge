# Internal follow-up notes — 2026-10-28

Evidence: comparison-results.json, validation-results.json, validation/*.log; exact revisions are in metadata.

1. Establish a passing quality baseline. Fix existing rustfmt drift and strict Clippy diagnostics in a separate change, avoiding a large formatting diff mixed into behavior changes. Doc comment spacing, dead code, needless returns and test assertions are recurring diagnostics. Do not blanket-disable lints.
2. Repair public examples that import private parser types. Default, bytecode and watchers builds expose doctest failures; test_access masks this through visibility changes. Preserve the public contract; approval is required if exposing new types is proposed. Prefer examples using existing public builder APIs.
3. Debug processing builds a whole-document log across instructions. In the 8 MiB normalization case RSS is much greater than normal execution. Investigate a separate bounded or writer-based debug API after public-contract approval; benchmark formatting separately from terminal rendering and retention. Preserve the existing method behavior.
4. Existing legacy APIs do not route through TextBuffer. Do not attribute baseline timing noise to buffered improvements. Future experiments should separately compare process_all_buffered and process_batch_buffered with explicit threshold, disk filesystem/cache state, spill bytes, syscall/I/O accounting and materialization boundary. Buffered batch is sequential whereas legacy batch is Rayon parallel, so document the scheduling tradeoff.
5. Add broader workloads before recommending default changes: fallback-heavy operations, expansion/repetition, empty documents, context/control flow, varying batch concurrency and datasets larger than RAM under isolated resource limits. Do not run risky memory-pressure cases on the user's host without explicit scope.
6. Improve measurement precision with repeated paired sessions beyond the current 100-execution wrap samples, confidence intervals, retained compiler/dependency fingerprints and CPU affinity when available. Current GNU time resolution is 10 ms; small-case percentage changes are not trustworthy optimization evidence.
7. Compiler upgrade attribution remains open: only 1.99.1 is installed. Run identical locked commits and full feature matrices on the user's prior compiler and 1.99.1 before labeling existing lint or doctest failures as compiler regressions. Do not silently install an arbitrary earlier compiler.
8. Existing tests/benchmark.rs includes wall-time and debug-ratio thresholds. These may be flaky under CI contention; separate correctness from performance tracking and compare distributions rather than failing unit tests on host-dependent fixed limits.

The initial benchmark changed no production implementation, dependency, version or public contract. The subsequent adapter implementation is described below.


## Standard-method adapter follow-up

The user authorized feature-gated replacements of process_all, process_batch and
process_all_with_debug using the experimental engine. Signatures and default
feature behavior are preserved. Batch keeps Rayon scheduling rather than
reusing the sequential buffered-batch scheduler. Debug spools its full log and
compares byte-for-byte against the legacy implementation, both plain and forced
color, before benchmark timings.

New evidence is in buffered-comparison-results.json and buffered-validation/.
The full test_access suite passes after updating the resource-limit regression
expectation: native repetition succeeds on disk but final String materialization
correctly refuses allocation. Existing doctest visibility and formatting/Clippy
issues remain inherited. Clippy emits no diagnostics in the new buffered adapter
module or its unit tests.

Priority next investigations: configurable adapter spool placement/threshold;
avoid needless disk roundtrips around fallback-heavy transforms; bounded
streaming implementations of replace and Unicode uppercase only after validating
whole-document semantics across chunk boundaries; reduce input duplication at
String API boundaries without changing the public contract; per-instruction
I/O and temporary-space accounting; sample a larger native-only workload before
recommending any default change. Preserve disk/RAM tradeoffs in user-facing
conclusions rather than equating lower process RSS with lower total host memory.

Legacy debug uses unresolved template parameters for instruction labels and block
name lookup. Named blocks other than the default x can fail when formatting the
log even if normal execution succeeds. This existed before the adapter; do not
silently change labels or error semantics in a performance experiment. Add a
separate behavioral regression and correct it in a follow-up authorized fix.


## Watcher adapter follow-up

A watcher-specific adapter now runs token.apply_buffered for each instruction,
then constructs the existing Arc<str>-based observer contexts. It deliberately
keeps legacy lookahead, report/reset and halt semantics. Eight pure diagnostics
cover all context fields, with parsed JSON equivalence and independent numeric
references. Results and current source fingerprints are in watchers-comparison-results.json;
full validation details are in watcher-validation-results.json.

The current WatcherContext contract prevents bounded-memory callbacks: arbitrary
closures can inspect and retain complete snapshots. Avoid inferring that spooled
transform storage will reduce observer memory. A truly streaming watcher design
needs a separate public-contract proposal and user approval (capabilities,
streaming reducers, optional lookahead and explicit snapshot materialization).
Do not silently replace existing callback inputs with partial chunks.

Known inherited issues for separate fixes: WatcherList.add_to_result pushes an
empty map once per watcher but fills the counter-indexed map, leaving surplus
empty rows for multi-watcher lists. The benchmark preserves the exact array shape
and compares every entry. The original watcher loop also lacks the early-halt
check present in process_all; changing that behavior would invalidate a strict
comparison. Report documentation claims an object whereas the serializer writes
a vector/array. These require dedicated regression tests and focused fixes.

Potential optimizations must be measured: drop transient transform outputs before
callbacks, avoid duplicate full-document snapshots where semantics permit,
reduce repeated full-text scans via opt-in combined reducers, serialize observer
reports via complete writes, and separate heavy diagnostic scans from engine
cost when interpreting CPU. Preserve explicit output-equivalence checks.


## Report reliability and scheduling follow-up

The first two priorities are now implemented: one map per observed step,
prepared callbacks, static-name result keys, bounded metadata reuse, streaming
ordered JSON and publish-after-flush using a same-directory temporary file.
The prior trailing-empty-row and unchecked short-write issues are fixed. Older
benchmark exports retain their historical representation. Current comparative
checks normalize only that known legacy padding, then compare all useful data.

The baseline for this experiment is the complete saved working source from
before these fixes, not main: it includes the same buffered watcher adapter.
Both sides use buffered,watchers and the identical worker, so the experiment
isolates WatcherList improvements. New fixtures mix prefix/suffix, regex replace
and Unicode uppercase over 16/32 MiB with 64/256 instructions. All individual
step metrics and final output are validated. Collection controls separate
observer bookkeeping from full-document transforms and scans.

Use watcher-reliability/results.json and report.md for the measured tradeoffs.
Do not infer whole-pipeline acceleration from metadata-only controls. Full-text
snapshots and repeated scans remain dominant. The report exporter does not claim
power-loss durability or inode-permission preservation; buffered publication
prevents partial success at the Write/flush/rename boundaries covered by tests.


## Steps 3–5 follow-up

Preparation is processor-owned and opt-in at the instruction trait. Stateless
built-ins bind simple literal parameters once; dynamic markers and nested tokens
stay unprepared. Clone invalidation is required because get_transform_vec returns
publicly mutable templates. Invalid preparation must not fail registration or
execute previously unreachable instructions. Regression tests cover all of these.

Shared counts use exact function-item TypeIds to preserve the existing generic
set_watcher API and avoid unreliable function-address equality. Arbitrary closures
and fn pointers are deliberately opaque. The common character/word/line metrics
share one pass; singleton metrics keep optimized std implementations. The first
standalone calibration on the 32 MiB fixture showed the fused kernel slightly
faster than separate standard counts; the published shared-statistics control
measures the actual WatcherList path. No global cache retains user text.

The retained-text path is private and used when complete materialization is
already required. String adapters retain fallback ownership; watcher adapters
start from existing Arcs, avoid tempfile roundtrips and reuse unchanged snapshots.
Public file placement, explicit spill, UTF-8 validation and safe resource-limit
failure tests remain. WatcherContext and report schema are unchanged.

Use steps-345/results.json and report.md for final measured tradeoffs. Baseline
already includes steps 1–2, so this is not a comparison against main. Step 3–4
also apply to non-buffered APIs; ranking the updated original engine against the
updated experimental engine requires a distinct comparison if needed later.
The large pipeline data still include repeated no-op regex/uppercase operations;
other instruction mixes, function-pointer registries, nested blocks or dynamic
parameters may have smaller gains. Keep any speed and RAM conclusions scoped to
the measured cases rather than claiming a universal performance improvement.
