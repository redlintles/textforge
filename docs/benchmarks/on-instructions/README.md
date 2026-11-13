# `on*` instruction benchmark

Reproduce:

```sh
cargo build --release --example on_instructions_benchmark
python3 scripts/benchmark_on_instructions.py \
  --binary target/release/examples/on_instructions_benchmark --samples 5
```

Files: [environment](environment.json) · [summary](summary.json) ·
[raw samples](raw.json) · [summary before the buffer-reuse change](summary-before-buffer-reuse.json)

## Setup

- Input: `data/book1.txt`, `book2.txt`, `book3.txt` repeated to 104,704,960 bytes
  and 2,395,840 lines (LF, mostly ASCII with some multibyte text).
- Release build, default features, `process_all` (borrowed input, so the result
  is always copied once into an owned `String`).
- One fresh process per sample, 5 samples per case, median reported. Time
  excludes reading the file and parsing the pipeline. Peak RSS is the process
  `VmHWM` and includes the 100 MB input and the 100 MB output.
- Machine: 2 vCPU Intel Xeon @ 2.10 GHz, Linux 6.18, rustc 1.97.0. The numbers
  describe this machine and workload only.

## Results

| Case | Pipeline | Median (s) | Peak RSS (MiB) |
| --- | --- | ---: | ---: |
| `identity` | `null;` (cost of `process_all` itself) | 0.058 | 204.8 |
| `raw_whole` | `raw a A;` | 0.220 | 204.8 |
| `onlni_whole` | `onlni 1 2395840 apply raw a A;` | 0.342 | 204.8 |
| `onlni_tenth` | `onlni` over 10 % of the lines, from the middle | 0.160 | 214.8 |
| `onln_middle` | `onln 1197920 apply raw a A;` | 0.124 | 204.8 |
| `onln_ten` | ten `onln` spread over the document | 0.717 | 204.9 |
| `onck_middle` | `onck 50000000 50000999 apply raw a A;` | 0.097 | 204.8 |

### Buffer reuse (applied)

The first measurement assembled every changed section into a new buffer. For a
section covering the whole document that meant input + nested result + output:

| Case | Before (s / MiB) | After (s / MiB) |
| --- | ---: | ---: |
| `onlni_whole` | 0.410 / 304.6 | 0.342 / 204.8 |
| `onlni_tenth` | 0.154 / 214.9 | 0.160 / 214.8 |

Growing the nested result's own buffer removes the extra copy and 100 MiB of
peak memory for large sections; small sections are unchanged within noise.

## Reading the numbers

- **Locating lines dominates.** `onlni_whole` costs about 0.12 s more than
  `raw_whole`: one scan of 100 MB with the shared line scanner
  (`src/utils/lines.rs`, `char_indices` per character). `onln_middle` scans half
  the document (~0.065 s above `identity`), and `onln_ten` repeats such a scan
  ten times (~0.66 s). The section work itself is small.
- **Memory stays flat.** No case allocates more than the selected section on top
  of the input and output; repeated `onln` does not accumulate memory.
- **`onck`** costs one `char_indices` scan up to `end` (~0.04 s for 50 M
  characters here).

## Decisions taken from these numbers

1. **Native buffered implementation: not implemented.** Memory is already
   bounded by input + output + section in the whole-document fallback; a native
   spool version would only matter together with a streaming line scanner.
2. **Line/char index caches (0.3.0 F6 entry condition):** the measurements show
   a clear time gain is available for *repeated* `on*`/`*ln` on large documents
   (ten `onln` = 0.72 s, almost all scanning). A cache would store one offset per
   line — 2.4 M lines × 8 bytes ≈ 18 MiB for this input — and its build cost is
   one scan (~0.13 s). Whether that trade is worth a profile field is a
   maintainer decision; a faster byte-level scanner (as explored in the
   [line performance audit](../line-performance/report.md)) would reduce the
   same cost for every line instruction without extra memory and is the cheaper
   first step.
