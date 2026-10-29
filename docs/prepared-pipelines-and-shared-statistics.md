# Prepared constants, shared statistics and retained text

This implements steps 3, 4 and 5 after the report reliability/scheduling fixes.
It adds no dependency and changes no bytecode, extension, version or callback
signature. The existing trait-object architecture and linear execution remain.

## 3. Prepare unequivocally constant bindings

Registered processor-owned TokenWrapper instances can retain a separately bound
instruction. All stateless built-in transforms, plus null/end, opt in through
the new default-false InstructionMethods::supports_preparation method. Custom
instructions keep per-execution binding unless their author explicitly opts in
and guarantees reusable immutable state.

Preparation runs at registration for text files, builders and bytecode-loaded
pipelines. Only simple literal strings/numbers qualify. Any variable reference,
interpolation delimiter (including escaped or malformed markers), or nested-token
parameter keeps its runtime path. Instruction groups and context-dependent
bindings are not recursively frozen. Validation failures are deferred to normal
execution, preserving failures after halt and unused invalid instructions.

The public token template and parameter serialization stay unchanged. Cloning a
wrapper discards the private prepared binding because get_transform_vec exports
clones whose public token field can subsequently change. Only the private
registered instance keeps the cache. Tests count bindings across repeated runs,
change variables/expressions, mutate exported templates and check deferred errors.
Prepared instructions retain additional bound metadata; the benchmark includes
registration and measures the RAM cost rather than assuming it is free.

## 4. Share standard statistics without changing callbacks

WatcherList recognizes the exact function-item types of current_len_chars,
word_count, line_count, char_delta and word_count_delta during registration.
TypeId compares Rust type identities; function addresses are not compared.
Direct imports/aliases of these items qualify. Function pointers, custom
closures and wrappers remain opaque and execute normally, including side effects.
Replacing a callback clears or updates its shared-statistics identity.

Each step computes only requested current/before statistics. A single requested
count uses the standard-library implementation; combined character/word/line
requests share a Unicode-aware pass. Delta watchers reuse those current counts
and share a pass over before when necessary. No complete document or Arc is
retained by the statistics object after the call. Callback/report ordering and
string-valued results are unchanged.

Tests compare all combinations against chars().count(), split_whitespace() and
lines(), including empty input, CRLF, lone CR, trailing LF, vertical tabs,
non-ASCII whitespace, combining characters and ZWJ emoji. Other default watchers
continue using their existing implementations.

## 5. Avoid unnecessary materialize/spill cycles

TextBuffer now has an internal retained-text path for owned Strings and shared
immutable Arc<str> snapshots that already have to exist. Public constructors and
explicit file APIs keep their threshold/spill policy. String-returning standard
processor adapters can retain a fallback result instead of immediately writing
it to a tempfile and reading it again at the next fallback.

Watcher processing starts from its existing Arc snapshot without a deep copy or
tempfile write. Compatible fallback transforms borrow that snapshot; unchanged
borrowed results reuse it. Owned results transfer their allocation, and final
snapshot conversion can reuse the previous Arc when text is identical. Native
prefix/suffix/repeat operations support retained text; explicit spill remains
available and commits a replacement storage only after successful writing.

Retaining a required materialization does not impose a global RAM bound. Arbitrary
watchers still receive and may retain complete snapshots. The lower-memory
explicit file/buffer APIs remain available. Regex/uppercase and other transforms
retain whole-document Unicode semantics; no unsafe chunk-based approximation
was introduced.

## Verification and reproduction

```sh
cargo test --features test_access
python3 scripts/benchmark_steps345.py
```

The saved alternative after steps 1–2 is compared against these changes, with
identical release buffered,watchers builds and an identical worker. It reuses
16/32 MiB fixtures and 64/256-instruction mixed pipelines, and adds a large
process_all case plus controls for constant regex bindings and standard counts.
Every output and every report is validated independently and compared byte for
byte. Source archives, hashes, samples, compressed reports and validation logs
are in docs/benchmarks/steps-345/. The report states medians, ranges, CPU and peak
RSS, including any regressions; it does not claim that each individual change
accounts for the entire combined gain.
