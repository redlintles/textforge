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
