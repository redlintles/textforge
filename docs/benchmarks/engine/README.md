# Engine benchmark (0.3.0)

Compares the engine before the 0.3.0 engine work (`f103578`, the tip of
`feat/0.3.0` after the `src/tokens` audit) with the optimized engine
(`4f50512`). The instructions themselves were not changed by this work; it
covers what surrounds them: loading, parameter binding, variable resolution,
`{{ }}`/`[[ ]]` interpolation, expression evaluation and the processor's
per-run setup.

## How it was measured

- Harness: `examples/engine_benchmark.rs` (public API only, so the same file
  builds on both revisions). Each case prints the median of its samples.
- Driver: `scripts/benchmark_engine.py`, fresh processes, baseline and
  candidate alternating, one warm-up per binary and case, 3 rounds of 3
  samples, pinned to one CPU. Raw numbers: [`results.json`](results.json).
- Machine: AMD Ryzen 5 4600G, Linux 7.0.8 (Docker `rust` image),
  rustc 1.100.0, `--release --features bytecode`.
- Profiles: callgrind (perf events are not available inside the container).
  Callgrind instruction counts were also used to check the cases whose wall
  times differ by less than the machine's noise.

To reproduce:

```sh
git worktree add ../base f103578 && cp examples/engine_benchmark.rs ../base/examples/
(cd ../base && cargo build --release --features bytecode --example engine_benchmark)
cargo build --release --features bytecode --example engine_benchmark
python3 scripts/benchmark_engine.py \
  --baseline ../base/target/release/examples/engine_benchmark \
  --candidate target/release/examples/engine_benchmark \
  --output docs/benchmarks/engine/results.json
```

## Results

| Case | What it runs | Before (s) | After (s) | Speed-up |
|---|---|---:|---:|---:|
| `expression_params` | 600 steps with constant `[[ ]]` parameters, 2,000 runs | 1.0541 | 0.0165 | 63.8× |
| `template_params` | 200 steps with `{{x}}` templates (`ate`, `rfw`), 2,000 runs | 0.3772 | 0.0543 | 6.9× |
| `blocks` | 120 `blk` lines and 2 `cblk`, 5,000 runs | 0.2253 | 0.0375 | 6.0× |
| `variables_churn` | 800 `var`/`mutv`/`val`/`eval` steps, 2,000 runs | 0.3066 | 0.0569 | 5.4× |
| `iter_expression` | `iter 20000 times onln '[[__COUNTER__ % 3 + 1]]' apply tua`, 20 runs | 1.1142 | 0.2872 | 3.9× |
| `load_bytecode_5k` | load 5,000 instructions from bytecode, 20 times | 0.3093 | 0.1500 | 2.1× |
| `variable_params` | 600 steps with `{{x}}`/`{{n}}` parameters, 2,000 runs | 0.1482 | 0.0762 | 1.9× |
| `small_runs` | 3-step pipeline, 200,000 runs | 0.0442 | 0.0371 | 1.2× |
| `long_literal_pipeline` | 300 literal steps, 2,000 runs | 0.0294 | 0.0286 | ≈ |
| `parse_text_5k` | parse 5,000 text instructions, 20 times | 0.1412 | 0.1392 | ≈ |
| `iter_literal` | `iter 20000 times tua`, 20 runs | 0.0198 | 0.0195 | ≈ |
| `raw_100mb` | `raw a A` over 100 MB | 0.1887 | 0.1857 | ≈ |
| `batch_64x1mb` | `process_batch`, 64 files of 1 MB | 0.3431 | 0.3456 | ≈ |

"≈" marks differences inside the run-to-run noise. Callgrind confirms them:
instructions executed before → after are 1.164e9 → 1.173e9 (`parse_text_5k`,
+0.8 %), 2.817e8 → 2.841e8 (`iter_literal`, +0.8 %) and 3.508e8 → 3.523e8
(`long_literal_pipeline`, +0.4 %); the wrapper carries two more cached fields.
Whole-document and batch cases are dominated by the instructions and file I/O,
which this work did not touch.

## What changed (one commit each)

1. **Compiled templates** (`a90b24c`, `src/utils/template.rs`). Interpolation
   used to scan each string parameter with `INTERP_RE` and build the evalexpr
   operator tree of each `[[ ]]` on every run. Registered pipelines now compile
   each dynamic parameter once into literal text, variable reads and
   expressions with prebuilt trees, and keep the expected parameter kinds, so
   binding no longer queries `TOKEN_TABLE`. Expressions that read no variable
   and call no function are evaluated once, and text whose templates are
   constant is bound once like any literal. A differential test checks the
   compiled form against `interpolate_with` on edge cases and 20,000 generated
   inputs.
2. **Small expression context** (`1c8d23f`). Each evaluation built a
   `HashMapContext` (SipHash, a `String` key per variable). Expressions read a
   few names, so a short vector scanned linearly replaces it; registered
   functions go through `GlobalFunctions::call`, still the single extension
   point.
3. **Context instructions bound once** (`a9241a7`). `var`, `val`, `mutv`,
   `eval`, `blk`, `cblk` and `ifdc` hold immutable bound state, so they opt into
   preparation; `eval` keeps its compiled expression and `blk` no longer
   deep-copies its payload on each run.
4. **Bytecode decoded from memory** (`2416f29`). Each bounds check of the
   streaming reader asked for the file's length and position (two system calls
   per record and parameter). Files up to 64 MiB are read once and decoded from
   a `Cursor`; larger ones are still streamed.
5. **Lazy unknown-pipeline error** (`5928d4d`). Every successful pipeline
   lookup formatted an error message it then dropped.
6. **Memo of the last dynamic binding** (`5bcf870`). An instruction with
   dynamic text or number parameters (`raw {{p}} X`) was rebound, and its
   regex recompiled, on every run. Instructions that opt into preparation now
   keep their last binding and reuse it while the resolved values repeat.
   Concurrent runs that find it busy bind on their own.
7. **Compile only dynamic parameters** (`4f50512`). Plain literal instructions
   skip compilation, keeping load time where it was.

## Behavior checks

- The full pre-commit checklist (fmt, clippy with and without `test_access`,
  tests for every feature set; 1480 tests with `test_access`) passed after each
  commit.
- A scratch differential (not committed) ran 4,000 random pipelines mixing
  variables, `eval`, templates, escapes, stray delimiters, failing expressions,
  blocks, `iter`/`onln`, `rpt`/`padl` and `end` on three inputs, twice each,
  through both revisions: all 19,100 results (14,476 outputs and the error codes
  and inputs of the rest) are identical.

## Where the remaining time goes

- `iter_expression`: about a third of the time is evalexpr evaluating its
  tree (it allocates while evaluating). The rest is rebinding `onln` on each
  iteration, because its count changes every time and it holds a nested
  instruction, which the memo does not compare.
- Loading: compiling regexes for `raw`/`rfw` when instructions are prepared,
  which is needed once per instruction.
