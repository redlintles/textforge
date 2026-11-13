# Static context and built-in variables benchmark

Reproduce (from the repository root, with a baseline checkout of the previous
commit in `../base`):

```sh
python3 scripts/benchmark_builtin_variables.py --baseline ../base --rounds 5
```

Files: [environment](environment.json) · [summary](summary.json)

## Setup

- `examples/builtin_variables_benchmark.rs`, release build, default features.
  One process per sample, five rounds, baseline and current runs alternated;
  each process reports the median of five in-process repetitions.
- Large cases: `data/book*.txt` repeated to 100,072,310 bytes (CRLF line
  endings, a few non-ASCII characters). Time excludes building the input.
- Baseline: `feat/0.3.0` at `a75f804`, before the static context existed.

## Results (seconds, median)

| Case | Pipeline | Baseline | Current |
| --- | --- | ---: | ---: |
| `small_runs` | `tbs; atb '>'; ate '<';` × 200,000 runs on 7 bytes | 0.0363 | 0.0395 |
| `expression_iter` | `iter 2000 times ate '[[__COUNTER__ % 10]]';` × 50 | 0.2248 | 0.2251 |
| `variable_iter` | `var x = 'a'; iter 2000 times ate '{{x}}';` × 50 | 0.0303 | 0.0304 |
| `null_100mb` | `null;` | 0.0551 | 0.0561 |
| `raw_100mb` | `raw a A;` | 0.1827 | 0.1823 |
| `lines_100mb` | `ate '{{__LINES__}}';` | — | 0.0870 |
| `len_chars_100mb` | `ate '{{__LEN_CHARS__}}';` | — | 0.0617 |
| `lines_twice_100mb` | `ate '{{__LINES__}} [[__LINES__ + 1]]';` | — | 0.0919 |

## Reading the numbers

- **Pipelines that do not read built-ins** run as before. Parameter binding
  (`expression_iter`, `variable_iter`) and whole-document work are unchanged
  within noise.
- **Each run** clones the pipeline's shared static context (`Arc`): about
  16 ns per `process_all` call (181 → 197 ns for a three-instruction pipeline
  on 7 bytes). Building a new static context per run cost 53 ns; it is now
  built when the pipeline is registered or renamed. Runs that read a file
  (`process_file`, `process_batch`) build one with the file path.
- **`__LINES__`** costs about 30 ms per 100 MB above `ate` alone (≈3.3 GB/s),
  and **`__LEN_CHARS__`** about 5 ms. Reading the same built-in several times
  in one parameter list scans once (`lines_twice_100mb`).
- The counts use separate passes over 255-byte blocks with `u8` counters,
  which the compiler vectorizes. Measured alternatives on the same 100 MB:
  `filter().count()` 36 ms, `windows(3)` 37 ms, `chars().count()` 29 ms,
  against 6–10 ms per blocked pass. A first byte-by-byte scanner cost 150 ms.
