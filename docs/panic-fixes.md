# Panic and allocation failure fixes

Scope: the four crash paths reproduced in the October 27, 2026 exploratory audit. Each fix has its own commit and regression tests. No bytecode protocol, opcode, extension, or public signature changes are intended.

## 1. Empty textual instruction

- Reproduction: a pipeline containing only `;` previously indexed an empty token vector and panicked.
- Change: obtain the instruction name with a checked lookup and return the existing TextParsingError on missing names.
- Regression coverage: bare/whitespace/escaped-newline instruction lines; valid comments and Unicode parameters remain accepted.
- Validation results are recorded below before committing. Repository-wide format, Clippy and normal-feature doctest failures were already reproduced before these fixes; they are not silently treated as passing checks.

### Step 1 validation

Focused parser regressions: 2 passed. Full checklist (offline dependency cache):

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

The full test_access suite passed. The other failures match the pre-existing formatting, Clippy and two private-parser doctest failures documented in the audit. No unrelated files were reformatted.

## 2. Declared bytecode record sizes

- Reproduction: `u64::MAX` in instruction size caused capacity overflow; forged parameter/padding sizes also requested allocations before file bounds were checked.
- Change: validate declared records against physical remaining bytes and parameters against their enclosing instruction; use checked usize conversions and fallible buffer reservations. Seek over validated padding instead of allocating a throwaway buffer. Existing padding and protocol remain supported.
- Regressions: maximum instruction size, excessive parameter size, padded consecutive instructions, and Unicode serialization/deserialization.

### Step 2 validation

Four focused bytecode regressions passed, including serialization/deserialization and preserved padding. Full checklist:

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

The full test_access suite passed; the same pre-existing repository checks remain failing. Instruction-count reservation is intentionally addressed in the next separate fix.

## 3. Untrusted bytecode instruction count

- Reproduction: `u32::MAX` in a tiny header previously attempted to reserve about 160 GiB of instruction storage and aborted under a process memory limit.
- Change: require enough physical bytes for the minimum record count, start with an empty instruction list, and grow it fallibly only as records are decoded. No arbitrary maximum pipeline length is imposed.
- Regressions: maximum count in a tiny file, inconsistent nonzero count, and a valid empty pipeline. Existing valid Unicode/padded round trips continue to run.

### Step 3 validation

All seven focused bytecode regressions passed. The release CLI replayed the original 186 bytecode mutations and 300 textual fuzz inputs under a 512 MiB virtual-memory limit: **486 cases, no panics or aborts**, only success or normal error exits. Full checklist:

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

The full test_access suite passed; pre-existing format/Clippy/doctest failures remain. Trailing bytes and compatible padding retain their original handling.

## 4. Repetition size and allocation failure

- Reproduction: output-size multiplication overflow panicked; a legitimate large expansion aborted when the allocator refused its infallible allocation.
- Change: checked byte-size multiplication and `String::try_reserve_exact` return existing ValidationError values. Empty/zero/one repetitions preserve behavior. The reserved buffer is filled by doubling complete input copies, keeping UTF-8 boundaries valid and avoiding a second infallible allocation.
- No hard output limit or new public API is introduced. Fallible reservation handles allocator refusal; it cannot prevent OS-level termination under memory overcommit or guarantee that unrelated allocation paths are fallible.
- Regressions: arithmetic/capacity overflow, huge repetition of empty input, 455 Unicode/count comparisons, and a Linux subprocess with a 256 MiB address-space limit requesting 800 MiB of repeated output.

### Step 4 validation

All 14 Rpt unit tests passed, including 455 Unicode/count comparisons. The Linux allocation-refusal subprocess regression passed. Under the same 256 MiB virtual-memory limit, the baseline release CLI aborted (SIGABRT), while the fixed release CLI returned exit 1 with ValidationError and no partial output.

Release end-to-end repetition benchmark (three runs per version, same machine and inputs; median seconds; output hashes matched in all 18 runs):

| Workload | Baseline | Fixed |
|---|---:|---:|
| 2-byte Unicode input repeated 1,000,000 times | 0.006131 | 0.005782 |
| 64 KiB input repeated 1,024 times | 0.103657 | 0.093516 |
| 16 MiB input repeated 4 times | 0.112903 | 0.113219 |

These local end-to-end samples support keeping the doubling fill strategy; they do not establish universal performance gains. Full pre-commit checklist:

- `cargo fmt --check`: exit 1.
- `cargo clippy --all-targets -- -D warnings`: exit 101.
- `cargo clippy --all-targets --features test_access -- -D warnings`: exit 101.
- `cargo test`: exit 101.
- `cargo test --features bytecode`: exit 101.
- `cargo test --features watchers`: exit 101.
- `cargo test --features test_access`: exit 0.

Final test_access total: **948 passing tests**, including doctests. The existing format/Clippy and two private-parser doctest failures remain outside this crash-fix scope. No dependencies, public signatures, protocol versions, or opcode values changed. The four fixes are committed separately on `codex/fix-panic-paths`; earlier uncommitted skill/reference changes are intentionally excluded.
