# Python binding rules audit

Reviewed on 2026-10-30 against AGENTS.md section 17. Core baseline: `4e7c6d0`;
Python binding baseline: `4a97f68` (package 0.1.1), an independent repository at
`src/bindings/python`. The vendored core marker is
`7314cfc9d421de004ed5140dcf8276dd177601d0`. No changes to the public/portable API
were found between that marker and the reviewed parent baseline.

This is a current compliance review. The new rules cannot establish which
requirements governed earlier development, or why a developer omitted a feature.
No binding source, dependencies, versions, or release settings were changed.

Paths below start at `src/bindings/python/` unless labeled as parent paths.

## Rule-by-rule assessment

| Rule | Assessment | Evidence and remaining work |
| --- | --- | --- |
| 1. Independent projects | Met | Separate `.git`, Cargo workspace, Cargo version, and package metadata. Parent `Cargo.toml` excludes the nested crate and parent `.gitignore` ignores it. Binding history contains separate implementation, fluent-builder, metadata, and release-fix commits. |
| 2. Portable API only | Met in production | `src/lib.rs` imports `textforge::api::portable` and delegates execution to it. No second engine or parser was found. The test-only Rust oracle uses the public fluent API and `test-reference`; this is not the production adapter. |
| 3. One-to-one public API parity | Not met | All 58 instruction helpers plus three block/conditional helpers are represented. The processor surface is incomplete; see the gap inventory below. `Pipeline.compile()` and `Processor.register()` provide idiomatic construction/registration, but there is no complete method-level mapping for the processor and builder terminal operations. |
| 4. Language conventions | Met in reviewed surface | Explicit `snake_case` methods, Python exceptions, `PathLike[str]`, typed immutable plans, and composition over private PyO3 objects. Signature/keyword-argument tests exist. |
| 5. Core evolution | Partially met | `vendor/CORE_REVISION`, snapshot-refresh instructions in `docs/architecture.md`, and generated-builder/oracle freshness checks exist. The pinned API is not currently behind the parent API. However, freshness checks cover the vendored builder, not a complete processor/binding parity map; the current gaps remain. No automatic upstream synchronization is required by the rule. |
| 6. Typing | Met | Public annotations, `_native.pyi`, `py.typed`, strict mypy configuration, and CI checks. The reviewed wheel contains stubs and the typing marker matching the checkout. |
| 7. Human approval | Not verifiable historically | Source files and commit messages do not establish that a maintainer explicitly approved each binding update. Manual publishing environments are documented, but publishing approval does not prove development approval. This audit makes no unauthorized-development finding. |
| 8. Documentation, tests, commits | Partially met | README, architecture/builder/development/release guides, pytest, doctests, lint/type checks, and focused commits exist. `docs/releasing.md` still says the distribution starts at 0.1.0, while `Cargo.toml` and the reviewed wheel are 0.1.1; README's unpublished status cannot be verified from local files. Commit timing does not prove every historical checklist was executed. |
| 9. Compatibility matrix | Partially met | Core revision, Python >=3.10, abi3, enabled bytecode, disabled watchers/buffered, and tested wheel targets are documented across manifests, architecture, and release docs. CI targets Linux/Windows x86_64 and macOS arm64 with selected Python versions. There is no consolidated compatibility matrix or precise minimum supported Rust version beyond edition 2024. Local review establishes Linux/Python 3.12 results only. |
| 10. Errors and conversions | Met for exposed representations; parity incomplete | `error()` preserves portable code/stage/message/instruction/input. Integer extraction and Python helpers reject negative/oversized instruction numbers and bool arguments. Unicode, combining characters, NUL, and overflow tests exist. Paths use Unicode text only; byte paths and surrogate-containing native paths are explicitly unsupported, without lossy conversion. Exposing the portable native-path alternatives is still a rule 3 gap. |
| 11. Ownership, callbacks, concurrency | Partially met | Immutable/reusable plans, temporary lowering ownership, compiled-object independence, GIL release, and PyO3 borrow guards are documented. Tests cover plan reuse, construction callback exceptions, and shared compiled execution. No dedicated regression was found for conflicting mutable processor calls, resource cleanup after failed operations, or processor lifetime independence. Runtime observer/stream callbacks are unavailable and hence have no binding lifecycle/re-entrancy tests. Construction callbacks are a different, tested facility. |
| 12. Parity tests | Partially met | An independent Rust builder oracle covers 61 operation/group cases and CI checks its freshness. All 61 saved oracle cases are successful results; none compares a Rust error with a Python error. There is no systematic Rust-versus-Python comparison of error codes/stages/messages/context, complete processor method map, or coverage of missing watcher/buffered capabilities. Bytecode round trips have Python integration tests but not a complete differential processor oracle. |
| 13. Distribution validation | Mostly met; verification incomplete | CI builds and installs wheels, runs pytest on multiple platforms/selected Python versions, and rebuilds/tests an sdist outside checkout. The local wheel contains required typing files. CI has no explicit assertion that wheel typing metadata is present, and checkout-based mypy can pass even if distribution typing files were omitted. External workflow success and protected-environment settings were not verified. |
| 14. Clean, maintainable code | No concrete violation found | Thin Rust delegation, isolated argument conversions, explicit typed Python definitions, bounded lowering, and readable generated source with regeneration instructions/freshness checks support KISS/DRY and separation of concerns. The 751-line facade includes generated documented methods; its size alone is not evidence of a violation. Future processor/buffer/observer additions should preserve focused modules. |

## Public API parity gaps

- `show_transforms` is not exposed. Typed/list snapshots are available but do not
  reproduce its console-output operation.
- `process_all_with_watchers`, observer registration/scheduling, callback errors,
  and reports are not exposed; the watcher feature is disabled in the adapter.
- `process_all_buffered`, `process_single_buffered`, `process_file_buffered`, and
  `process_batch_buffered` are absent, as are portable buffers/options and stream
  reader/writer hooks; the buffered feature is disabled.
- Reusable `BatchPool` and `process_batch_in_pool` are not exposed. Python batch
  processing creates a new portable pool for every call.
- Portable capability discovery (`features()`) is absent.
- Native byte/wide path constructors are absent. Unicode-only paths are a
  documented restriction, not full native-path coverage.
- There are no direct `create_pipeline`, `add_transform`, or builder `build`
  entry points. `Pipeline()`, `Processor.register(plan)`, and `compile()` cover
  related workflows, but the differences and any equivalent mapping must be
  recorded explicitly before claiming complete parity.

The binding itself acknowledges most of these limitations in
`docs/architecture.md` under "Initial release scope" and in
`docs/development.md` under "Next work". They were documented deferrals rather
than concealed capabilities. They nevertheless fail the new full-parity rule.

## Verification performed

- The complete parent pre-commit checklist passed before the guidance/skill
  commit: formatting, clippy default/test_access, and tests default, bytecode,
  watchers, and test_access (including doctests).
- The updated project skill passed the skill-creator frontmatter validator.
- Installed the existing 0.1.1 abi3 Linux wheel from local `dist/0.1.1` into an
  isolated virtual environment using system pytest, without dependency downloads.
  All **97 Python tests passed** on CPython 3.12 / Linux x86_64. Tests imported
  the installed wheel; package source was not added to PYTHONPATH.
- Re-executed `scripts/generate_builder_reference.py --check` against the real
  vendored Rust builder to verify all **61 reference cases**.
- Inspected the wheel ZIP: `_native.pyi` and `py.typed` are included; the Python
  facade, argument helper, stubs, and marker match the reviewed checkout.
- Static review covered manifests, production Rust/Python source, stubs,
  generators, tests, CI, documentation, and recent Git history. No direct unsafe,
  unwrap, or expect calls were found in the adapter's production sources.

This audit did not rebuild release artifacts, rerun Ruff/mypy, execute remote CI,
verify publication state, or test Windows/macOS. Passing existing tests does not
resolve the coverage and parity gaps listed above.
