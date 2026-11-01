# TextForge 2.0.0 core release candidate

The requested core package version is **2.0.0**. This release preserves the
public Rust/portable contracts, instruction outputs and bytecode protocol of
revision `72fc7fc`; the major version number is not an announcement of intentional
breaking changes. See [changelog](../changelog.md) and
[optimization evidence](allocation-optimization.md).

## Scope

The candidate includes allocation optimizations, exact baseline fixtures,
regression tests, a reproducible benchmark runner, recorded wall/CPU/peak-RSS
measurements, and updated documentation. No dependency or license changes were
introduced. No binding repository was modified. The independent Python package
still targets its previous core snapshot; its distribution is not part of this
core package's validation.

## Validation

All commands below passed locally. `test_access` passed 1,268 tests/doctests,
including 949 unit tests; its two pre-existing informational benchmarks remained
ignored. The separate large-file benchmark completed 40 measured runs and eight
warm-ups with matching output checks. The checklist is:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features test_access -- -D warnings
cargo test
cargo test --features bytecode
cargo test --features watchers
cargo test --features test_access
cargo test --features buffered
cargo test --release --features test_access --test allocation_parity
cargo doc --no-deps --all-features
cargo package --allow-dirty --offline
```

The package command verifies an extracted distributable in a separate build. It
is run locally before the preparation commits; `--allow-dirty` permits the
not-yet-committed release files. Re-run `cargo package --offline` from the committed
clean worktree before publication if rebuilding the archive.

## Existing compatibility limits

The baseline fixtures retain existing index edge behavior, including six
empty-input arithmetic outcomes that differ between debug and release builds,
and `tucs` byte-offset matching. No new behavior fix is claimed for those cases.
The benchmark is a single-machine, single-worker in-memory experiment; it does
not certify foreign-language wheels, watcher overhead or buffered spill scaling.

## Publication

This preparation does not publish to crates.io, push Git refs or create a release
tag. Publication and remote release operations require a separate maintainer
action. The core `.crate` archive can be built with `cargo package --offline`;
the expected artifact is `target/package/textforge-2.0.0.crate`.
