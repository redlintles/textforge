# Allocation optimization in TextForge 2.0.0

This release keeps the existing instruction, fluent Rust, portable API and
big-endian bytecode contracts. It changes how transforms store intermediate text.
No instruction name, opcode, parameter type, extension or feature was changed.

## Implementation

The private `utils::allocation` module preserves `Cow::Borrowed` slices for
contiguous selections and reuses `Cow::Owned` storage for cuts and localized
replacement. Owned slices retain their allocation capacity; keeping a tiny
selection does not guarantee that the original allocation is returned to the OS.

- Character conversion and splitting no longer allocate a string per scalar:
  `tlcc`, `tlcs`, `tucc`, `tucs`, `splc`.
- Word operations write directly to the output instead of building vectors of
  owned words: `clw`, `ctr`, `cts`, `jcmc`, `jpsc`, `tlcw`, `tucw`, `jkbc`, `jsnc`.
  The shared capitalization helper no longer copies the complete word tail into
  a temporary string. `cfw` replaces only the initial scalar. `ctc` writes the
  selected chunk directly, preserving whitespace.
- `dlf`, `dll`, `dla`, `dlb`, `slt`, `sslt`, `tbs`, `tls`, `trs`, `dlna`,
  `dlnb` and `slnc` preserve borrowed slices or adjust owned storage.
- `dls`, `dlc`, `ins`, `atb`, `rnw`, `rlw`, `dln`, `dlni` and `rln` edit owned
  ranges. `rlw` retains only its final regex match instead of collecting all.
- `apl`, `pln` and `inl` share internal line insertion helpers instead of
  constructing temporary instruction objects; `apl` reuses owned text.
- `dle`, `dlm`, `klm` and `uniqln` allocate output only after an excluded line.
  `rmws` and `nln` compact owned text. `joinln` and `revln` retain trivial input.
- `rcw`, `htmle`, `htmlu`, `end` and empty `ate` preserve unchanged input.
  `rmape` and `rmapd` borrow map names rather than cloning them for lookup.
- `rtl` and `rtr` use byte cuts at scalar boundaries instead of `Vec<char>` and
  retain zero rotations. `padl`/`padr` avoid a full repeated padding temporary;
  `padr` appends directly. `rpt` reuses owned storage while retaining checked
  multiplication and fallible reservation. Trivial `rev` retains its input.
- `tla` and `tua` use in-place ASCII conversion; Unicode keeps the existing
  whole-string conversion. Unescaped `jsonu` validates through `serde_json` and
  retains its content range. Regex setup removes clones used only for borrowing.

The extra Unicode buffer in `jkbc`/`jsnc` is intentional: whole-string lowercase
has contextual behavior (Greek final sigma), which scalar-by-scalar lowercase
would change. `tlcw`/`tucw` retain whole-word conversion for the same reason.
`jsone`, nontrivial Unicode reversal, line reversal, regex replacement and
normalization can legitimately require new output. `raw`, `rfw`, `rmp`, `urle`,
`urld` and `unorm` already have useful unchanged-input paths.

## Compatibility evidence

`tests/data/allocations/baseline.json` contains 2,106 exact outputs, structured
error debug representations and legacy panic outcomes captured from revision
`72fc7fc`, for all 81 instructions, 13 input strings and both ownership modes.
Inputs include empty text, emoji, combining marks, case expansion, Greek sigma,
whitespace, CRLF and Unicode line terminators. Tests detect registry coverage
changes and exercise every benchmark case. Focused token tests assert borrowed
results and reuse of owned allocation pointers; private-helper tests cover ranges,
line filtering and Unicode capitalization.

The original `ctc`/`slt` empty-input arithmetic has six debug-versus-release
outcome differences in this corpus. The fixture stores both outcomes. This
performance release preserves those existing behaviors, as well as the legacy
byte-offset selection in `tucs`; it does not present them as newly corrected
Unicode/index behavior. Behavioral fixes need a separate compatibility decision.

Core/portable signatures and adapters were reviewed for impact; no adapter code
changes are needed for these internal optimizations. The independent Python
binding remains pinned to its previous core snapshot. These measurements and
checks establish core support only, not a tested Python 2.0.0 distribution.

## Benchmark method

The baseline is `72fc7fc`. Both versions build the exact same
`examples/allocation_benchmark.rs`, support module, case data, locked dependency versions,
release profile and `test_access` features. The harness explicitly calls the
registered `run_transform` in-memory path, so enabling buffered test helpers does
not silently switch the measured execution path.

The deterministic UTF-8 files contain approximately 4 MiB and 16 MiB of words,
emoji, combining characters, Greek letters, HTML characters, empty lines and
mixed logical lines. Each binary reads the same file. There are two workloads:

1. **coverage**: 81 registered pipelines, one for each instruction, with required
   context setup and a common valid-index guard. Every pipeline runs three times
   on a fresh large input. This exercises each instruction repeatedly without
   early selection leaving every later instruction a tiny document. Some
   instruction parameters deliberately select small outputs or make no change.
2. **large**: one 21-step pipeline run three times, keeping document scale through
   trims, no-match replacement, HTML/JSON round trips, character case conversion,
   capitalization, character splitting/removal, rotations and localized edits.
   It complements coverage by retaining large intermediate documents.

The runner uses fresh sequential processes, pins both binaries to the same
available logical CPU, warms up each binary/scenario once, and alternates order
across five measured runs. GNU `time` records wall time, user CPU, system CPU and
maximum resident set size. CPU seconds are user + system; CPU percentage is that
sum divided by wall time. RSS is process peak memory, not total allocated bytes.
Raw samples retain both process wall time and the harness's inner elapsed time.

Input bytes, output bytes and deterministic FNV-1a output digests must match for
every run. This digest is an additional workload consistency check, not a proof
against collisions; exact small-corpus comparisons and normal tests provide the
behavioral regression checks. File reading, setup and output hashing are included
in process metrics. Builds and warm-ups are excluded. These are single-machine
observations, not universal speed or memory guarantees; allocator behavior and
input composition affect the result. Watcher overhead, spilling, batch scaling
and foreign-runtime adapters are outside this benchmark.

## Reproduction

From the candidate worktree:

```sh
mkdir -p /tmp/textforge-baseline
git archive 72fc7fc | tar -x -C /tmp/textforge-baseline
# Copy only the harness and its input descriptions to the archived baseline:
mkdir -p /tmp/textforge-baseline/examples/support /tmp/textforge-baseline/tests/data/allocations
cp examples/allocation_benchmark.rs /tmp/textforge-baseline/examples/
cp examples/support/allocation_workload.rs /tmp/textforge-baseline/examples/support/
cp tests/data/allocations/cases.json /tmp/textforge-baseline/tests/data/allocations/
cargo build --release --features test_access --example allocation_benchmark
cargo build --manifest-path /tmp/textforge-baseline/Cargo.toml --release \
  --features test_access --example allocation_benchmark
python3 scripts/benchmark_allocations.py \
  --baseline /tmp/textforge-baseline/target/release/examples/allocation_benchmark \
  --candidate target/release/examples/allocation_benchmark \
  --output docs/benchmarks/allocations-v2.json
```

Create the empty `/tmp/textforge-baseline` directory before extracting. Only the
candidate manifest needs the `required-features` example declaration; the
baseline command enables the feature explicitly. Dependencies must be available
locally or fetched before benchmarking. The runner requires Linux, Python 3 and
GNU `time`, and adds no Cargo dependencies.

Generate baseline fixtures with the same example's `fixtures` mode in debug and
release; retain `outcome_debug` only when it differs. Do not regenerate fixtures
from the optimized implementation, which would erase the comparison oracle.

See [raw benchmark results](benchmarks/allocations-v2.json) and the release
summary below for measured medians and limits.

## Measured results

Machine: AMD Ryzen 5 4600G with Radeon Graphics; Linux; logical CPU 0. rustc 1.99.1 (58cf8f9c3 2026-10-13). Five measured processes per version/scenario, three document runs per process; medians below.

| Input / workload | Version | Wall seconds | CPU seconds | One-core CPU % | Peak RSS MiB |
| --- | --- | ---: | ---: | ---: | ---: |
| 4 MiB / coverage | 72fc7fc | 5.36 | 5.36 | 99.8 | 221.2 |
| 4 MiB / coverage | 2.0.0 | 2.59 | 2.58 | 99.6 | 25.5 |
| 4 MiB / large | 72fc7fc | 1.93 | 1.92 | 100.0 | 220.9 |
| 4 MiB / large | 2.0.0 | 0.20 | 0.19 | 100.0 | 21.3 |
| 16 MiB / coverage | 72fc7fc | 23.11 | 23.09 | 100.0 | 867.8 |
| 16 MiB / coverage | 2.0.0 | 11.09 | 11.08 | 99.9 | 99.5 |
| 16 MiB / large | 72fc7fc | 8.08 | 8.07 | 99.9 | 867.6 |
| 16 MiB / large | 2.0.0 | 0.90 | 0.89 | 98.9 | 85.5 |

- 4 MiB / coverage: **2.07×** wall-time speedup, **88.5%** lower peak RSS. Wall ranges: baseline 5.32–5.38s; candidate 2.57–2.61s.
- 4 MiB / large: **9.65×** wall-time speedup, **90.4%** lower peak RSS. Wall ranges: baseline 1.91–1.95s; candidate 0.19–0.20s.
- 16 MiB / coverage: **2.08×** wall-time speedup, **88.5%** lower peak RSS. Wall ranges: baseline 22.96–23.33s; candidate 11.03–11.12s.
- 16 MiB / large: **8.98×** wall-time speedup, **90.1%** lower peak RSS. Wall ranges: baseline 8.07–8.13s; candidate 0.89–0.91s.

CPU utilization remains near one fully used core; the improvement is fewer CPU
seconds to finish the same work, rather than a claim of low CPU utilization.
All 40 measured runs passed the output-byte/digest consistency checks.
