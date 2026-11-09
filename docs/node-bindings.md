# Node.js binding repository

The Node.js adapter is an independent Git repository at `src/bindings/node`:
https://github.com/redlintles/textforge-node

It is excluded from this Rust workspace and ignored by this repository. Commit
the Node.js implementation, package metadata, CI and its engine snapshot in that
nested repository, and commit only Rust-core integration or engine changes here.
It is not a submodule: cloning TextForge does not fetch the Node.js repository.

To get the binding next to the core:

```sh
git clone https://github.com/redlintles/textforge-node src/bindings/node
cd src/bindings/node
```

The package `textforge-node` (0.1.0, unreleased) is a typed N-API addon built
with napi-rs. It targets Node.js 22 and 24 and ships ESM and CommonJS entry
points with TypeScript declarations. It builds from its own vendored copy of
the 0.2.0 core (`vendor/textforge`), pinned by a hash manifest and patch that
track core revision `701c5b2`. It does not depend on this checkout.

See that repository's `README.md`, `docs/compatibility.md` and `docs/parity.md`
for supported platforms, API parity and development commands.
