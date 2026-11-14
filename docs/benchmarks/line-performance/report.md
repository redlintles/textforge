# Performance audit of the line instructions

Date: **2026-10-31, America/Sao_Paulo**. Audited revision:
`3e87c17b623f470be93df9b1f4fcb3300efe167a` (TextForge 0.2.0).

[Summary and priorities](../../line-performance-summary.md) ·
[Full timings](timings.json) · [Timing samples](timings-raw.json) ·
[Memory](rss.json) · [Additional experiments](extras-timings.json) ·
[Additional memory](extras-rss.json) · [Environment](environment.json)

## Conclusion

**Optimizing the current implementation is justified; replacing the whole
engine with ropes, line lists or an edit tree is not, for now.** Large gains come
from searches limited to the range needed, block copies and use of the
`Cow`/`String` ownership the existing contract already allows. This audit did
not change `src/`, the public APIs, the bytecode or the Python binding.

Two caveats rule out a blanket "do everything in place" recommendation:

- `revln` has a memory risk proportional to the number of lines; a compact index
  proved a better compromise than always reading the text backwards.
- Truncating an owned buffer can keep almost the whole document in RAM. For
  large cuts, copying only the small part kept can be the better choice.

## 1. What happens when line 2 is deleted

The document is one contiguous UTF-8 `String`. Lines are temporary slices
`(&str, &str)`, content and terminator. There are no thousand line objects whose
numbers must be updated one by one.

The current implementation of `dln`:

1. Reads the whole text to count the lines and validate the index.
2. Reserves a new buffer with the capacity of the input.
3. Walks the lines again, copying the surviving ones into that buffer.

So **one deletion is O(N), not O(L²)**: there is no reallocation per line. There
is one new output buffer and a copy of the surviving bytes. For a large ordinary
input, input and output coexist and the peak approaches two documents.

In an owned string, `String::drain` can compact the suffix in the same buffer.
That still shifts the following bytes once: deleting near the start remains
O(N). With a borrowed input and an interior cut, one `Cow<str>` cannot represent
two disjoint slices; building a contiguous output requires a copy.

For **1,000 lines of 64 bytes**, `dln 2` cost **126.14 µs** in the current
version and **1.19 µs** in the prototype with a short search and a two-block
copy. This does not mean every 1,000-line file costs that: the size in bytes and
the line lengths matter.

Repetition is a separate risk: K successive deletions cost roughly O(KN), or more
precisely O(Σ Nᵢ). Deleting every line one at a time can behave quadratically in
the initial size.

## 2. Method and limits of the measurements

- `--release --locked` build, Rust 1.99.1, Linux x86_64, AMD Ryzen 5 4600G.
  Each process was pinned to the CPU recorded in [environment.json](environment.json).
- 1,610 samples in the main matrix and 150 additional ones: five samples per
  configuration, in an order shuffled with a fixed seed. Tables show medians;
  the JSON files include quartiles, minimum, maximum and the individual samples.
- 100 operations per small sample, two at 8 MiB and one at 64 MiB; in the
  additional studies, 20 small operations or one large one.
- Building the data, preparing the token/regex in the main matrix and acquiring
  the owned string are **outside** the transform time. Dropping the output is
  also outside; internal releases of the input are inside.
- `borrowed`: `Cow::Borrowed`; `owned`: an owned buffer with initial capacity
  equal to its length; `spare`: an owned buffer with 256 extra bytes.
- Memory uses fresh processes: 171 peak samples through `/usr/bin/time`, plus
  24 additional ones (three per configuration). The peak includes creating the
  input. RAM after the operation comes from `/proc/self/smaps_rollup`; `String`
  capacity is measured separately.
- ASCII data alternates `keep`/`drop` lines with distinct content per line.
  The `^keep` filters select or remove about half of the lines. `dle` finds no
  empty lines in this set; `uniqln` finds only unique lines.
- Unicode with every terminator, text without line breaks, repeated lines and
  text made only of LF were measured too. These are not samples of real
  production files, and the machine was not dedicated to the benchmark.
- Disk, file decoding, Python conversion, callbacks, watchers and full
  processor latency were **not** measured. The additional chains call the
  kernels directly, with the same text result, without the interpreter.
- `malloc`/`realloc` calls were not counted: capacity and pointer identity are
  not an allocation counter. No `unsafe` hook was used.

A short search that returns a slice does not read a 64 MiB file from disk in
microseconds: it works on input already in memory. APIs that return `String`
still materialize a borrowed slice at the return boundary.

### Semantic check of the experiments

The harness compared the candidates with the real tokens on **14,642 inputs ×
two ownership modes = 29,284 cases**, including empty input, all eight
terminators, atomic CRLF, emoji, combining characters and UTF-8 sequences that
look like separator prefixes. It also compared every valid range of each input,
and deletion chains with an equivalent range deletion.

Check: [semantic-check.json](semantic-check.json). The candidates are
experiments, **not ready patches**: their errors use generic labels. Production
code must still preserve codes, messages, context, error precedence, preparation,
watchers, buffered processing and API parity. The matrix does not prove
performance on other systems, allocators or regex patterns.

## 3. Current timings of the 16 instructions

Times in **ms**, borrowed input. Sizes are document bytes, terminators included.
Indices at the end and whole-text selections also appear in the JSON; short
search gains shrink when almost all of N must be crossed. Selections and cuts
are near the start (line 2 or lines 2–3); inserted and replacement texts are
small.

| Instruction | 1,000 lines × 64 B (62.5 KiB) | 8 MiB / 131,072 lines × 64 B | 64 MiB / 16,384 lines × 4 KiB |
| --- | ---: | ---: | ---: |
| `apl` | 0.064 | 12.674 | 83.603 |
| `dln` | 0.126 | 20.191 | 128.462 |
| `dlna` | 0.063 | 8.311 | 50.956 |
| `dlnb` | 0.064 | 12.415 | 83.903 |
| `slnc` | 0.074 | 9.324 | 66.656 |
| `dlni` | 0.128 | 19.609 | 127.628 |
| `inl` | 0.073 | 13.237 | 99.757 |
| `nln` | 0.069 | 12.390 | 81.149 |
| `rln` | 0.137 | 21.687 | 131.625 |
| `pln` | 0.074 | 13.392 | 100.267 |
| `dle` | 0.076 | 13.578 | 93.093 |
| `klm` | 0.088 | 13.684 | 66.584 |
| `dlm` | 0.088 | 13.755 | 65.920 |
| `revln` | 0.070 | 15.689 | 83.857 |
| `uniqln` | 0.154 | 50.520 | 126.501 |
| `joinln` | 0.080 | 13.398 | 95.401 |

Do not compare the columns as if the number of lines were fixed: lengths and
counts change. More lines also mean more per-line calls, a larger reversal index
and a larger deduplication table.

## 4. Inventory of the problems

N = input bytes; L = lines; U = unique contents; P = bytes up to the range;
R = surviving bytes; T = inserted or replacement text; S = join separator.
"Scan" below refers to the line scanner; copying bytes is an additional cost.

| Token | Current work and storage | Diagnosis and direction |
| --- | --- | --- |
| `apl` | Finds the first terminator, walks to the last one, copies the input and may grow the output | Drop the search for the last terminator: test the suffix. Reserve the result once; reuse `Owned`. |
| `dln` | Counts N, then walks N again; output with capacity N | Find only the two boundaries; copy prefix and suffix in blocks, or one `drain` on `Owned`. |
| `dlna` | Counts N even when two lines are kept; then finds the prefix | Stop at the requested line. Borrow the prefix, or choose between truncation and a small copy. |
| `dlnb` | Counts N, finds the start and copies the suffix | Find only the start. A borrowed slice or one compaction of the suffix. |
| `slnc` | Counts N, walks to the end of the range and reserves N even for a tiny R | Bounded search; exact output or a view. Avoid keeping a large buffer in `Owned`. |
| `dlni` | Counts N and rebuilds N | Same fix as `dln`, applied to one range; do not run several small drains. |
| `inl` | Counts N, finds the point and style, copies N+T | Bounded search and one insertion in an owned buffer. At EOF it still delegates to `apl`, with more scanning and temporary metadata. |
| `rln` | Counts N, finds the style, rebuilds N; reserves N+T without subtracting the removed content | Find one range and replace it as a block; return the input if the content is identical. |
| `pln` | Builds a temporary `Inl` and counts all of N to insert at line 1 | No need to validate that the first line exists. Use a private insert/prepend helper. |
| `nln` | Reads N and copies the contents; copies even when there are no line breaks | Fast path when nothing changes; `retain` on `Owned` can lower the peak but needs a look at retained capacity, and still scans. |
| `dle` | Reads N and copies every kept line; always allocates | Allocate on the first discarded line; keep the `Cow` if no line is empty. |
| `klm` | Reads N, runs the regex per line and always reserves an N-sized output | The literal regex is already prepared; the likely gain is a lazy output and returning the input when everything is kept. |
| `dlm` | Same, copying the lines that do not match | Same care as `klm`; do not recompile the regex per line or change match semantics. |
| `revln` | `Vec<(&str,&str)>` plus an N-sized output | Each descriptor takes 32 B on this target. Reduce to offsets or iterate backwards under a memory constraint. |
| `uniqln` | HashSet of slices, O(U) metadata, and an N-sized output | It does not duplicate each line in the set. Hashing and the memory for U remain necessary in the current strategy; a lazy output helps when there are no duplicates. |
| `joinln` | Reads N and reserves N; the output can be larger with a long S | The result is the sum of the contents + max(L−1, 0)×S. Large growth can be inherent; an exact reservation needs another pass. |

Building temporary `Inl`/`Apl` tokens also clones parameters and payload at run
time, despite the outer token being prepared. Private helpers avoid this. The
benchmark used a small payload; it did **not** measure large inserted texts.

### Failures and operations that change nothing

`check_line` and `check_range` copy the input into the error context. On failures
over large documents this can create another N-sized copy. I do not recommend
removing that context or changing messages for performance; they are observable
contracts. The success path can stop early, counting everything only when it has
to produce the existing error.

*(Fixed in 0.3.0: errors now keep an excerpt of the text; see
[the 0.3.0 audit](../../audits/0.3.0-on-debug-audit.md), item 7.)*

`dlnb 1`, `dlna last`, `slnc 1 last`, filters that keep everything and other
cases that change nothing also return a new string today. In long pipelines,
keeping the `Cow` between steps avoids several copies; the external API can still
require one owned string at the end.

## 5. Experiments and demonstrated gains

### 5.1 Bounded search and reuse of the owned buffer

8 MiB, 64-byte lines; **ms per transform**:

| Token | Current, borrowed | Prototype, borrowed | Current, owned | Prototype, owned |
| --- | ---: | ---: | ---: | ---: |
| `apl` | 12.674 | 3.965 | 12.883 | 0.002 |
| `dln` | 20.191 | 4.033 | 20.258 | 0.653 |
| `dlna` | 8.311 | 0.001 | 8.918 | 0.001 |
| `slnc` | 9.324 | 0.001 | 9.568 | 0.002 |
| `nln` | 12.390 | 12.367 | 12.909 | 9.988 |

For `dln` with borrowed input, the gain comes from removing the full scanner
passes and copying two blocks, **not** from making the interior cut zero-copy.
The peak stayed close to two documents. With 64 MiB of owned input, the
prototype lowered the peak from about **131 MiB to 67 MiB** on the 64-byte-line
set, shifting the suffix once in the existing buffer.

`apl` went from **83.603 to 32.662 ms** on 64 MiB borrowed. With `owned`, the
allocator on this machine could grow the buffer in place, and the observed time
was about 5 µs. That favorable case depends on the allocator and available
space; **it is not a guarantee of O(1) append**. Reserving in advance or using
existing capacity avoids the unnecessary growth seen in the current version.

`nln` without line breaks, on 8 MiB borrowed, went from **10.511 to 6.102 ms**,
returning the same input. With line breaks present, the borrowed version was
practically even; the owned version gained more moderately. There is no basis to
promise a large CPU gain for every filter just because it works in place.

### 5.2 A naive optimization can keep 64 MiB

Selecting lines 2–3 of 64 MiB, 4 KiB lines, **owned** input:

| Strategy | Median time | Output length | Output capacity | Peak RSS | RSS after returning |
| --- | ---: | ---: | ---: | ---: | ---: |
| Current `slnc` | 71.062 ms | 8 KiB | 64 MiB | 66.7 MiB | 2.9 MiB |
| Bounded search + truncate/drain | 0.011 ms | 8 KiB | 64 MiB | 66.6 MiB | 66.7 MiB |
| Bounded search + exact copy | 5.092 ms | 8 KiB | 8 KiB | 66.6 MiB | 2.7 MiB |

The current output reserves a lot of virtual memory but touches few pages.
Truncating reuses pages of the input that were **already touched** and keeps them
resident. The exact-copy prototype has to release the large buffer inside the
call; that allocator/kernel cost shows in its time. Even so it was about 14×
faster than the current version here, without keeping the whole document.

The same risk appeared in `nln` with 8 MiB of LF and an empty output: the current
version kept 2.8 MiB resident after the call, while `retain` in the owned buffer
kept 10.7 MiB. The peaks were close to 10.6 MiB. Avoiding an allocation does not
mean minimizing the memory the output keeps, even when the output is empty.

**Recommendation:** return a slice for borrowed input; for an owned buffer, reuse
it when the cut is small and consider an exact output or releasing capacity when
the result is much smaller. A conditional `shrink_to_fit` is another hypothesis to
measure, not a result shown here. Do not shrink or remove repeatedly inside a
per-line loop.

### 5.3 Reversal: memory versus speed

Additional timings in **ms**, borrowed input:

| Document | Current: 32 B descriptors per line | Compact index: 8 B offsets per line | No index: reverse scanner |
| --- | ---: | ---: | ---: |
| 1,000 lines / 62.5 KiB | 0.072 | 0.073 | 0.121 |
| 8 MiB / 64 B lines | 15.366 | 13.572 | 18.568 |
| 64 MiB / 4 KiB lines | 83.000 | 81.338 | 121.689 |
| 8 MiB / LF only (8,388,608 lines) | 238.433 | 145.600 | 78.866 |

Peak RSS on 8 MiB made only of LF:

| Strategy | Median peak RSS |
| --- | ---: |
| Current | 274.7 MiB |
| Compact offsets | 82.8 MiB |
| No index | 18.7 MiB |

The current `Vec` stores **slices**, not independent strings, but that is four
words per line on the measured target: 32 × 8,388,608 = 256 MiB of metadata
before the output. There is also excess capacity and amortized reallocation of
the vector for sizes that are not powers of two. With 1 GiB of LF alone, that
would be about 32 GiB **of descriptors alone**; this is arithmetic
extrapolation, not an OOM test.

The offset index cuts that cost by four per entry and kept the time close to the
current one on long lines. The ~2% difference on that set is within the observed
variability and does not support a promise of a CPU gain.

Reversal without an index was excellent in the dense case of empty lines, but
**about 47% slower** on long lines. So I do not recommend replacing it
everywhere. Offsets are a first change of moderate cost; an index-free path can be
used where limiting auxiliary memory is shown to be necessary. An adaptive choice
must measure the cost of deciding and of any extra passes.

### 5.4 Repeated deletions remain expensive

K deletions of line 2, 8 MiB, direct kernels, **ms for the whole chain**:

| K | Current chain | Chain with short search + reuse | One current `dlni 2 (K+1)` |
| --- | ---: | ---: | ---: |
| 10 | 175.948 | 11.297 | 19.410 |
| 50 | 823.447 | 43.658 | 19.360 |

Even the candidate does not remove the K suffix shifts. For 50 deletions, one
existing range deletion was ~42× faster than the current chain. Rewriting the
pipeline by hand is an immediate fix when the intermediate states need not be
observed.

Do not merge such steps automatically without considering watchers, debug output,
context, variables, interruptions and the logical position after each step. The
comparison measures direct constant operations; it does not prove the equivalence
of arbitrary pipelines.

### 5.5 Scanner, deduplication and joining

The experimental byte scanner, with no new dependency and keeping the eight
terminators, lowered the median time to read 8 MiB of ASCII from **8.228 to
7.070 ms** (~14%); Unicode from **7.691 to 6.639 ms** (~14%); 64 MiB of long
lines from **51.331 to 49.868 ms** (~3%). Removing whole passes helps more than
speeding up each pass. Do not use `str::lines()` as a replacement without adapting
the semantics: TextForge recognizes more terminators.

`uniqln`, on 64 MiB of distinct 64-byte lines, reached about **174 MiB of peak
RSS**: input, output and the set of unique contents. This audit measured no
deduplication candidate. Storing only hashes is not an exact replacement:
collisions require comparing the content. A lazy output can avoid the copy when
there are no duplicates, but does not remove the O(U) metadata.

The `joinln` test used a one-byte separator. With large separators the result
itself can be much larger than the input. Do not justify a second pass to compute
the exact capacity with the numbers of this small case alone; measure real
separator and length distributions first.

## 6. Buffered processing and API boundaries

None of the 16 instructions implements its own `transform_buffered`. The fallback
materializes the document and runs the string transform. **A spool threshold is
not a global RAM limit for these instructions.**

There is an important distinction in the current code:

- `process_all` and batches on the default buffered path use
  `retain_materialized`. Once materialized, the output can stay a string between
  steps; it is wrong to say that all these paths rewrite the spool after every
  instruction.
- An explicit `process_all_buffered` with an ordinary, non-retained buffer can
  materialize and write the result to the spool again on every fallback.
- `PortableProcessor::run_transform` uses `Cow::Borrowed` directly and converts
  to `String`; `process_all` delegates to the core method, whose path depends on
  the features.

These points were checked by reading the code, **not** by an I/O benchmark in this
audit. Cut, filter and join operations are candidates for incremental chunked
execution, but that requires terminators and UTF-8 that cross chunks, preserved
errors and integration with the existing infrastructure. A per-line regex may
need memory proportional to the longest line; reversal and exact deduplication
have further requirements. Do not promise universally constant RAM.

## 7. Priorities and implementation cost

The estimates are engineering judgment, not measured hours: a developer familiar
with the project, without public API changes or new dependencies. They include
Unicode/EOF/error regressions and the local feature matrix. Cross-platform CI and
authorized binding snapshot updates add work.

| Priority | Change | Indicative cost | Worth it? |
| --- | --- | --- | --- |
| High | Boundary helper, early stop and block copies in `dln/dlni/dlna/dlnb/slnc` | 1–3 days for the family and tests | **Yes.** Large measured gain, local design; small outputs need a capacity policy. |
| High | `revln` with compact offsets | 1–2 days | **Yes, for files with many lines.** Large peak reduction, with no material loss shown on the long set. |
| High | `apl/inl/pln/rln`: private helper, reservation and use of `Owned`, without temporary tokens | 2–4 days for the family | **Yes.** Append measured; insertion and replacement still need a candidate-specific comparison. |
| Medium | Fast paths and late allocation in `nln/dle/klm/dlm/uniqln/joinln` | 1–3 days for the common cases | **Probably.** `nln` without breaks was measured; filters and deduplication need their candidates and discard rates measured. |
| Medium/low | Safe byte scanner | 0.5–2 days | **After the redundant passes.** 3–14% gain on the large sizes measured, less dominant after short searches. |
| Conditional | Index-free or adaptive `revln` | 1–3 more days | **If RAM is the limit.** Better in the dense extreme, but can lose performance on long lines. |
| Conditional | Buffered overrides with incremental processing | One or more weeks per set, depending on the infrastructure needed | **If files exceed the RAM budget.** Real I/O must be measured; the microbenchmarks do not establish the full return. |
| Low for now | Rope, piece table or persistent indexed document | Weeks, affecting many transforms and observability | **Not justified by the current data.** The existing contract already allows most of the local gains. |

These estimates are not deadlines. For a workload of only small files and few
operations, the extra maintenance may be worth less than the tens of microseconds
saved. For files of several MiB, batch processing or long pipelines, the local
gains are large enough to justify a first focused round.

The operational return depends on how often each path runs. For example, the
measured difference of `dln` on 8 MiB borrowed was about **16 ms per operation**;
10,000 calls are about **160 s of accumulated transform time**, before disk, FFI
and concurrency. This is a kernel projection, not a production SLA.

## 8. What I would implement first

1. A private helper that finds the boundaries and stops as soon as it has them.
   On error, keep the current count, context and precedence, without changing
   constructors.
2. `dln`/`dlni` by copying a few blocks or one compaction; range cuts handled
   differently for `Borrowed`, `Owned` and large size reductions.
3. Append, prepend, insertion and replacement without instantiating other tokens
   during `transform`; reserve only what is needed and keep ownership when
   possible.
4. Replace the `revln` descriptors with compact offsets. Keep index-free iteration
   as an option to evaluate for memory-bound workloads, not as the default.
5. Measure and apply fast paths for operations that change nothing. Only then
   decide on the scanner, persistent indices, adaptive heuristics and streaming.

To accept the patches: compare output bytes and every error field, run the
text/bytecode/portable/watchers/buffered tests, watch retained capacity and RSS as
well as latency, and repeat the benchmarks on real workloads. Changes to bindings
or public contracts follow the project's approval process; this exploration made
none.

## 9. Sources and reproduction

Main reading: [scanner and errors](../../../src/utils/lines.rs),
[dln](../../../src/tokens/transforms/dln/mod.rs),
[slnc](../../../src/tokens/transforms/slnc/mod.rs),
[apl](../../../src/tokens/transforms/apl/mod.rs),
[revln](../../../src/tokens/transforms/revln/mod.rs),
[uniqln](../../../src/tokens/transforms/uniqln/mod.rs),
[trait fallback](../../../src/tokens/mod.rs),
[TextBuffer](../../../src/api/text_buffer.rs),
[buffered processor](../../../src/api/processor/buffered.rs),
[portable processor](../../../src/api/portable/processor.rs).

```sh
cargo build --release --locked --example line_performance_audit
./target/release/examples/line_performance_audit verify
python3 scripts/run_line_performance_audit.py --binary "$PWD/target/release/examples/line_performance_audit"
python3 scripts/run_line_performance_extras.py --binary "$PWD/target/release/examples/line_performance_audit"
```

Reproduction needs Linux, `taskset`, `/usr/bin/time`, Python 3 and Rust. The
scripts rewrite the result JSON files. Small variations should be judged by the
quartiles; another machine will not produce the same numbers. The experimental
code lives only in
[examples/line_performance_audit.rs](../../../examples/line_performance_audit.rs).

### Checks run for the audit's artifacts

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo clippy --all-targets --features test_access -- -D warnings`
- `cargo test`, `cargo test --features bytecode`, `cargo test --features watchers`
  and `cargo test --features test_access`
- Release build and a run of `line_performance_audit verify`
- Ruff lint and format on both Python scripts; both were run to collect the data.

The core checks passed; the binding repositories were not changed.
