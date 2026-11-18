# `tryi` and error details (0.4.0)

What `tryi` costs, and whether pipelines without it got slower when errors
started to carry their trace and parameters.

- **Before**: `main` at 5204cff (0.3.2 with the `txf` fixes), release `txf`.
- **After**: the 0.4.0 revision, release `txf`.
- Machine: 2-core x86-64 VM, Linux. Inputs are written by the script.

Reproduce:

```sh
cargo build --release --features cli           # in each revision
python3 scripts/benchmark_tryi.py --before OLD/txf --after NEW/txf --runs 9 --out results.json
```

Wall times are medians of 9 runs of `txf` and include starting the process
(about 2 ms). Workloads with `tryi` only run on the new revision. Raw data:
[`results.json`](results.json).

## Results

| Workload | Before (s) | After (s) | Change |
|---|---|---|---|
| 20 MiB, five instructions | 0.205 | 0.210 | +2.5% |
| 3,000,000 iterations of tua | 0.089 | 0.089 | -0.8% |
| 3,000,000 iterations of tryi tua (succeeds) | — | 0.145 | — |
| 3,000,000 iterations of tryi dls 99 (fails, ignored) | — | 2.651 | — |
| 1,000,000 × cblk, 0 variables | 0.080 | 0.083 | +4.4% |
| 1,000,000 × tryi cblk, 0 variables | — | 0.210 | — |
| 1,000,000 × cblk, 10 variables | 0.089 | 0.088 | -1.0% |
| 1,000,000 × tryi cblk, 10 variables | — | 0.215 | — |
| 1,000,000 × cblk, 1000 variables | 0.093 | 0.089 | -4.8% |
| 1,000,000 × tryi cblk, 1000 variables | — | 0.218 | — |

The changes of ±10% on the 0.1 s workloads are run-to-run noise on this VM:
two runs of the same binary differ as much. Instruction counts (below) are
the reliable comparison.

## Instruction counts

`valgrind --tool=callgrind`, which counts executed instructions and does not
depend on the load of the machine:

| Workload | Before | After | Change |
|---|---|---|---|
| `iter 200000 times tua` in a block | 98,821,137 | 99,027,773 | +0.2% |
| `iter 100000 times cblk c` (block `c`: `tua`) | 151,063,999 | 148,172,543 | −1.9% |
| `raw "a+" "{{n}}"` and `dlc 0 [[n - 1]]`, 50,000 times each on 270 KB | 24,332,570,136 | 24,333,211,943 | +0.003% |

```sh
valgrind --tool=callgrind txf --no-rc -f loop.textforge -i abc.txt
```

## What the numbers led to

1. **The first snapshot copied the variables, blocks and replacement maps**
   before each instruction that runs others. With 1,000 variables, 100,000
   `tryi cblk c` took 6.48 s (65 µs each): the copy, not the block.
   [`results-full-copy-snapshot.json`](results-full-copy-snapshot.json) holds
   that run (100,000 blocks, 300,000 iterations).
2. **Copy-on-write maps** (`Arc::make_mut`) made the snapshot free, but every
   change of a variable paid an atomic operation: 3,000,000 iterations of
   `tua`, without `tryi`, went from 0.079 s to 0.115 s (+46%), since `iter`
   changes `__COUNTER__` on each iteration.
3. **An undo journal** replaced both: a snapshot copies nothing; while it is
   open, the context records each value before its first change and puts them
   back on failure. Without an open snapshot a change costs one inlined check.
   `tryi cblk` costs the same with 0, 10 or 1,000 variables (about 0.13 µs
   more than `cblk`).
4. **callgrind** showed the check was a function call on every `__COUNTER__`
   update (+5.7% instructions on the `iter` loop); inlining it, with the
   recording out of line, brought the difference to +0.2%.

Ignoring an error costs about 0.9 µs (3,000,000 ignored `dls 99` in 2.7 s):
the error, its trace frame and its parameters are built, then dropped. A
pipeline that fails rarely does not notice; one that relies on failing in a
hot loop should test the condition instead (`ifdc`).
