# Allocation optimization parity oracle

Source revision: `72fc7fc` (the parent of the optimization work).

`cases.json` describes all 70 transforms and 11 logical instructions. Context
setup is included when necessary; `jsonu` is preceded by JSON serialization so
all benchmark inputs are valid. `baseline.json` records 81 × 13 × 2 = 2,106
outcomes. Output strings and the complete structured error debug representations
are compared exactly. Ownership modes are tested independently.

The original revision was built in both debug and release using the candidate's
unchanged fixture-generator source, with `test_access`. `outcome` is the release
result; `outcome_debug` is present only for the six differing legacy arithmetic
outcomes. `panic: true` records a baseline panic, not a new behavior requirement
for a future bug-fix release. This optimization preserves it rather than changing
invalid-index behavior accidentally. Golden data must be regenerated from the
explicit baseline revision, never from the candidate under test.

Commands after copying the example, support module and case data to an archive
of the baseline:

```sh
cargo run --manifest-path /tmp/textforge-baseline/Cargo.toml --features test_access \
  --example allocation_benchmark -- fixtures > /tmp/debug.json
cargo run --manifest-path /tmp/textforge-baseline/Cargo.toml --release --features test_access \
  --example allocation_benchmark -- fixtures > /tmp/release.json
```

Merge corresponding rows, retaining a row's debug outcome as `outcome_debug` only
when it differs from its release outcome. Verify that case, input and ownership
fields match before merging. Tests additionally check the case names against the
actual registry so added instructions cannot silently escape coverage.
