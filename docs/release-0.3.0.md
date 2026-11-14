# TextForge 0.3.0 core release

The core package version is **0.3.0**, published to crates.io on 2026-11-14.
Unlike 0.2.0, this release has breaking changes; the
[changelog](../changelog.md) lists them first, followed by the behavior changes
an existing pipeline may notice.

## Scope

- Specification work: static context and built-in variables, section
  instructions (`onln`, `onck`, `onlni`), file instructions (`wtf`, `apf`,
  `ppf`, `inf`), pipeline joins (`ppjn`, bytecode parameter type 0x05),
  execution profiles with `.txfrc`, nested debug traces.
- The `src/tokens` audit ([report](audits/0.3.0-tokens-audit.md)) and the
  on/debug audit ([report](audits/0.3.0-on-debug-audit.md)), all items closed,
  including safety limits for `rpt`, `padl`, `padr` and `iter` and bounded error
  inputs.
- Engine optimizations ([benchmark](benchmarks/engine/README.md)).
- Packaging: `cli` feature for the `txf` program, `include` list (the package
  is 1.4 MiB, 260 KiB compressed, instead of 204.8 MiB), `rust-version` 1.89,
  keywords, categories and docs.rs features. `SyntaxToken` and
  `TextForgeParamTypes` are `#[non_exhaustive]`.
- No binding repository was modified. The Python and Node.js bindings still
  vendor the 0.2.0 core and are versioned separately.

## Validation

All of the following passed on the release revision (`d8c05c2`) (Docker `rust:1-bookworm`,
rustc 1.100.0): 1,491 tests and doctests with `test_access` (two informational
benchmarks ignored), the allocation parity corpus in release mode, rustdoc with
`-D warnings` for the default and the docs.rs feature sets, a check with rustc
1.89 (the declared minimum), `cargo package` (builds and verifies the extracted
archive) and `cargo audit` (no advisories in 68 dependencies).
`cargo semver-checks` against the published 0.2.0 reports exactly the breaking
changes listed in the changelog and nothing else requiring a major bump.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features test_access -- -D warnings
cargo test
cargo test --features bytecode
cargo test --features watchers
cargo test --features buffered
cargo test --features cli
cargo test --features test_access
cargo test --release --features test_access --test allocation_parity
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --features bytecode,watchers,buffered,cli
cargo +1.89 check --all-targets --features test_access
cargo package
cargo semver-checks --baseline-version 0.2.0
cargo audit
```

`cargo package` warns that the benchmark examples are not in the package; that
is intended (they read data sets that stay in the repository).

## Publication

The release was published from the committed, clean release revision with:

```sh
cargo package            # target/package/textforge-0.3.0.crate
cargo publish
git tag -a v0.3.0 -m "TextForge 0.3.0" && git push origin v0.3.0
```

The binding repositories still vendor the 0.2.0 core; they move to 0.3.0 in
their own repositories.
