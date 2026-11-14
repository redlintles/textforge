# Line performance audit: summary

[Full report](benchmarks/line-performance/report.md) ·
[Results and method](benchmarks/line-performance/environment.json)

This audit measured TextForge 0.2.0 (2026-10-31).

**Optimizing the current implementations is worth it. Rewriting the engine with
a rope or a persistent list of lines is not justified for now.**

- **Deleting a line does not reallocate a thousand line objects.** The text is
  contiguous; `dln` currently scans twice and builds another string. One deletion
  is O(N). On 1,000 lines of 64 bytes it measured 126 µs; the candidate based on
  boundaries and block copies, 1.2 µs. That is transform time, without loading or
  writing the file.
- **On large files, copies matter.** With 64 MiB of owned input, `dln` went from
  133 ms to 7.2 ms on long lines. In the memory test with short lines, the peak
  fell from about 131 MiB to 67 MiB. The suffix is still shifted once; it did not
  become O(1).
- **`revln` deserves special attention.** On 8 MiB of line breaks only, its index
  pushed the peak to 275 MiB. Compact offsets brought it to 83 MiB; no index, to
  19 MiB. The index-free version was slower on long lines, so it should not
  replace the current path everywhere.
- **Searching only the part needed is the simplest gain.** `dlna`/`slnc` count the
  whole file even when they use only the first two lines. Stopping early removes
  that work; the return strategy must take ownership into account.
- **Reusing buffers does not always save RAM in the end.** Selecting 8 KiB of an
  owned 64 MiB string with `truncate` kept about 67 MiB resident. A short search
  plus an exact copy was 14× faster than the current version and released the
  large buffer. Reserved capacity is not the same as physical RAM.
- **There are unnecessary copies when nothing changes**, plus temporary tokens
  built in `pln` and when inserting at EOF. These are possible local improvements.
- **`uniqln` also uses memory per unique line.** The set already stores slices,
  not independent copies of each line. Removing that memory while keeping exact
  deduplication and order is not a trivial trade-off.
- **Many small deletions add up.** On 8 MiB, 50 `dln 2` cost 823 ms; one existing
  `dlni 2 51`, 19 ms. The rewrite is only worth it when intermediate states,
  watchers or other observations are not needed.
- **Buffered processing can still materialize everything.** The spool threshold
  does not limit the RAM of these instructions globally; that requires dedicated
  streaming work.

## Suggested order

1. Short searches, block copies and careful use of `Cow`/`Owned`.
2. A compact index in `revln`, and append/insert helpers without temporary tokens.
3. Fast paths for operations that change nothing, and late allocation in filters.
4. Then a faster scanner, streaming and heuristics driven by real workloads.

The first families are estimated at **a few days each**, with tests; streaming and
a new document representation cost much more. These are engineering estimates,
not promised deadlines. For few operations on small texts, all the sophistication
may not pay off; for several MiB or many operations, the measured local gains
justify the change.

**1,760 timing samples and 195 memory samples** were collected. The prototypes
were checked on 29,284 input/ownership combinations, plus chain comparisons.
Local measurements, in release mode and on one pinned CPU; they exclude I/O,
Python and full processor latency. No production implementation or binding was
changed. The candidates must still preserve the error contracts in full before
they can become production patches.
