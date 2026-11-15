# Line parser benchmark (0.3.1)

`split_line` (`src/text/line_parser.rs`) against `shell_words::split`, over the
918 lines of every `.textforge`/`.tft` file in the repository that use no new
escape, 2,000 rounds, best of 5, release build.

Reproduce: `scripts/benchmark_line_parser.py`.

| Run | `split_line` | `shell_words::split` | Speedup |
|---|---|---|---|
| 1 | 58.9 ns/line | 65.6 ns/line | 1.11× |
| 2 | 59.3 ns/line | 66.8 ns/line | 1.13× |
| 3 | 58.8 ns/line | 65.0 ns/line | 1.10× |

Machine: 2-core Linux container, Rust 1.97. Parsing runs once, when a pipeline
is loaded; the time is dominated by allocating the words. The parser copies runs
of ordinary characters as slices instead of pushing them one character at a time.

## Effect on a pipeline (48 MB text, 526,616 e-mail addresses)

| Pipeline | Median of 7 |
|---|---|
| 0.3.0 workaround: `iter 6 times raw "(?m)(^\|[ ])[a-z]+($\|[ ])" " ";` | 0.67 s |
| 0.3.1: `raw "\s+" "\n";` + `klm "^[A-Za-z0-9._+-]+@…$";` | 0.64 s |

The new pipeline reads the text once per instruction (split 0.44 s, filter
0.24 s measured separately) and, unlike the workaround, does not depend on the
other words being lowercase ASCII.
