# `txf` before and after the CLI fixes (0.3.2)

Compares the `txf` of `main` at `b3cb952` (before) with the head of
`fix/cli` (after), for [the CLI audit](../../audits/0.3.2-cli-audit.md).
Release builds, AMD Ryzen 5 4600G (12 threads), Docker `rust:1-bookworm`.

Reproduce: `scripts/benchmark_cli.py --base OLD_TXF --new NEW_TXF --sizes 1,50,200`.
The script generates log-like text (about 10% of the lines with multibyte
characters), runs each case with `-i FILE -o FILE`, and reports the median wall
time, the peak resident memory and whether both outputs are byte-identical.
Runs longer than 300 s are stopped. Raw results: the JSON files next to this page.

Pipelines: `upper` is `tua;`, `squash` is `raw "\s+" " ";`, `trim-mark` is
`tbs; ate ";";`.

Startup on a tiny input (median of 50): 5.51 ms before,
5.52 ms after.

## `--features cli` (the default install), 1 and 50 MiB, median of 5

| Size | Pipeline | Mode | Before | After | Peak memory before → after | Same output |
|---|---|---|---|---|---|---|
| 1 MiB | `upper` | `all` | 0.011 s | 0.011 s | 15 MiB → 16 MiB | yes |
| 1 MiB | `squash` | `all` | 0.022 s | 0.021 s | 16 MiB → 16 MiB | yes |
| 1 MiB | `upper` | `line` | 0.011 s | 0.011 s | 16 MiB → 16 MiB | yes |
| 1 MiB | `trim-mark` | `line` | 0.011 s | 0.011 s | 16 MiB → 16 MiB | yes |
| 1 MiB | `upper` | `chunk-65536` | 0.021 s | 0.011 s | 16 MiB → 16 MiB | yes |
| 1 MiB | `upper` | `chunk-4096` | 0.154 s | 0.011 s | 16 MiB → 16 MiB | yes |
| 50 MiB | `upper` | `all` | 0.285 s | 0.291 s | 105 MiB → 105 MiB | yes |
| 50 MiB | `squash` | `all` | 0.566 s | 0.673 s | 100 MiB → 101 MiB | yes |
| 50 MiB | `upper` | `line` | 0.301 s | 0.307 s | 105 MiB → 105 MiB | yes |
| 50 MiB | `trim-mark` | `line` | 0.321 s | 0.331 s | 101 MiB → 102 MiB | yes |
| 50 MiB | `upper` | `chunk-65536` | 23.541 s | 0.310 s | 155 MiB → 105 MiB | yes |
| 50 MiB | `upper` | `chunk-4096` | timeout (>300 s) | 0.311 s | — → 105 MiB | — (before timed out) |

## `--features cli`, 200 MiB, median of 3

| Size | Pipeline | Mode | Before | After | Peak memory before → after | Same output |
|---|---|---|---|---|---|---|
| 200 MiB | `upper` | `all` | 1.141 s | 1.229 s | 404 MiB → 405 MiB | yes |
| 200 MiB | `squash` | `all` | 2.269 s | 2.659 s | 387 MiB → 388 MiB | yes |
| 200 MiB | `upper` | `line` | 1.193 s | 1.203 s | 405 MiB → 405 MiB | yes |
| 200 MiB | `trim-mark` | `line` | 1.279 s | 1.314 s | 391 MiB → 391 MiB | yes |
| 200 MiB | `upper` | `chunk-65536` | timeout (>300 s) | 1.264 s | — → 405 MiB | — (before timed out) |
| 200 MiB | `upper` | `chunk-4096` | timeout (>300 s) | 1.216 s | — → 405 MiB | — (before timed out) |

## `--features cli,buffered`, 50 MiB, median of 5

| Size | Pipeline | Mode | Before | After | Peak memory before → after | Same output |
|---|---|---|---|---|---|---|
| 50 MiB | `upper` | `all` | 0.337 s | 0.333 s | 155 MiB → 155 MiB | yes |
| 50 MiB | `squash` | `all` | 0.695 s | 0.624 s | 151 MiB → 151 MiB | yes |
| 50 MiB | `upper` | `line` | 0.466 s | 0.490 s | 105 MiB → 105 MiB | yes |
| 50 MiB | `trim-mark` | `line` | 0.480 s | 0.500 s | 102 MiB → 102 MiB | yes |
| 50 MiB | `upper` | `chunk-65536` | 23.257 s | 0.352 s | 155 MiB → 105 MiB | yes |
| 50 MiB | `upper` | `chunk-4096` | timeout (>300 s) | 0.327 s | — → 105 MiB | — (before timed out) |

## Reading the results

- **Chunk mode** was quadratic: each chunk boundary was searched from the start
  of the text. It is now as fast as `all`, and uses less memory (the chunks are
  appended to one buffer instead of a vector joined at the end).
- **Every case that finished in both versions produced the same bytes.** The
  inputs have only LF line breaks, so the new line rules (finding 8) and the
  CRLF rule for chunks (finding 9) do not change these results.
- **Line mode** costs the same as before. Two intermediate steps were slower
  and were fixed before the end: passing the input file to every run built a
  new static context per line (`984a498`), and the shared line scanner
  decoded every character (`d1c5d5d`, which also speeds up `uniqln` and
  `__LINES__` by 2 to 4%). On a separate 50 MiB file (median of 7),
  `upper`, `line`, `--input`: 0.430 s before,
  0.547 s after finding 8, 0.489 s after `984a498`, 0.438 s after `d1c5d5d`.
- **`all` with `squash` looks 15 to 20% slower in the default profile; that
  difference is code layout, not the changes.** Bisecting with the same input
  (50 MiB, output to `/dev/null`, median of 9) gives 0.566 s at `b3cb952`,
  0.566 s at `15f14f2`, 0.671 s at `a580152` (which only changes how `main.rs`
  writes to standard output), 0.569 s at `984a498` and 0.716 s at `d1c5d5d`
  (which only changes `src/utils/lines.rs`, not used by `raw`). With
  `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1`, which removes most of that
  variance, the new `txf` is faster: `squash` 0.672 s → 0.563 s, `upper`
  0.264 s → 0.249 s. In the `buffered` build, `squash` is faster in the
  default profile too.
- **Writing the output atomically** (finding 16) costs nothing measurable:
  `-o FILE`, `> FILE` and `-o /dev/null` differ by the same amount in both
  versions.
