---
name: atp-project
description: Develop and review TextForge (formerly ATP), including Rust instructions, pipelines, bytecode, public and portable APIs, and language bindings under src/bindings. Use for TextForge/ATP code and binding work; do not use for unrelated Rust or binding projects.
---

# TextForge / ATP project

TextForge (formerly ATP) is a Rust text-processing DSL. The crate is edition 2024. Confirm current names, pipeline extensions, and feature gates in the code; legacy ATP names remain in some documentation.

## Work from the codebase

- Treat the repository as the source of truth for instruction names, signatures, opcodes, and behavior. `instructions.txt` is useful for orientation, but confirm details in the relevant module.
- Tokens live in `src/tokens/transforms/<mnemonic>/` and `src/tokens/instructions/<mnemonic>/`; each normally has `mod.rs` and `test.rs`.
- The common contract is `src/tokens/mod.rs::InstructionMethods`. Token lookup, syntax, and opcode registration are in `src/globals/table.rs`.
- Use the local instruction's `mod.rs` and neighboring tokens as the implementation template. Preserve Unicode-safe behavior where it is relevant.

## Token and bytecode changes

- Keep `to_atp_line`, `transform`, `get_string_repr`, `from_params`, and, when enabled, `get_opcode` and `to_bytecode` consistent with the token's syntax and registration.
- `to_bytecode` is feature-gated and returns `Result<Vec<u8>, AtpError>`. In tests that expect serialization to succeed, unwrap it: `let bytecode = token.to_bytecode().unwrap();`. Propagate or assert errors in production code instead of discarding them.
- Run bytecode-related tests with `cargo test --features bytecode` (or `test_access` when its helpers are needed). Do not change opcode values or bytecode layout without checking reader/writer compatibility.
- When adding or altering a token, update its module, its tests, `src/tokens/transforms/mod.rs` or `src/tokens/instructions/mod.rs` if needed, and the registration/syntax in `src/globals/table.rs`. Search for all call sites and documentation that name the mnemonic.

## Public API and conventions

- The Rust fluent API is defined by `TextForgeBuilderMethods` and related traits in `src/api/mod.rs`, implemented in `src/api/builder.rs`, `block_builder.rs`, and `conditional_builder.rs`, and run by `TextForgeProcessor` in `src/api/processor.rs`. Language adapters use `src/api/portable` directly.
- Builder operations return `Result`; examples and tests should unwrap only when success is part of the test precondition.
- Maintain existing public behavior unless the request explicitly changes it. Use `TextForgeError` and its error codes for validation failures rather than introducing ad-hoc error strings.
- Write new commits, code, and user-facing documentation in English unless the user requests otherwise.

## Language bindings

Before developing or reviewing a binding, read section 17, **bindings**, in the repository-root `AGENTS.md` (resolved from this skill as `../../../AGENTS.md`). It is the authoritative rule set; reload it for future binding tasks rather than relying on this summary. Read `docs/portable-api.md` and `docs/portable-parity.md` for the shared interop contract.

- Each `src/bindings/{language_name}` is an independent crate and Git repository with its own versioning. Inspect its status and use its own worktree and commits; parent Git status does not show binding changes.
- Route production calls exclusively through the portable layer. Mirror the public processor and builder functionality one-to-one using idiomatic names and types; implement missing shared capabilities in the portable layer, subject to contract approval, without duplicating engine logic.
- Core changes require reviewing API and binding parity. Keep a method mapping, supported core revision/version, runtime/platform/architecture/feature matrix, and Rust-reference parity tests for results and errors. Missing functionality remains a parity gap even if documented.
- Use native documentation, tests, type annotations/stubs for dynamic languages, and focused frequent commits. Validate shipped typing metadata, installed wheels/packages, and independent source rebuilds in CI for supported configurations.
- Preserve structured portable errors and lossless Unicode/path/numeric conversion. Document and test ownership, cleanup, callbacks, re-entrancy, and concurrency guarantees; identify unavailable facilities.
- Follow KISS, YAGNI, DRY, separation of concerns, and single responsibility pragmatically. Favor explicit, readable code and focused functions over speculative abstractions. Generated methods must remain readable and reproducible with CI freshness checks.
- Binding updates require explicit human-maintainer approval before changes. A read-only review is not approval to repair findings. Existing session authorization applies to the requested scope; do not ask again for already authorized actions. Publishing, pushes, dependency additions, version bumps, and public-contract changes follow `AGENTS.md` approval rules.
- For audits, report evidence for every rule, distinguish partial coverage from failures, and mark historical approval or external CI state as unverified when repository evidence cannot establish it. Apply newly introduced rules as current compliance criteria, without claiming they governed earlier development.

## References

- For designing or registering an instruction, read [instruction design](references/instruction-design.md).
- For a requested C-compatible binding layer, read [FFI planning](references/ffi-guide.md). The FFI is planned; do not implement it unless asked.
