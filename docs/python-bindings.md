# Python binding repository

The Python adapter is an independent Git repository at `src/bindings/python`:
https://github.com/redlintles/textforge-python

It is excluded from this Rust workspace and ignored by this repository. Commit
Python implementation, package metadata, CI, and its engine snapshot in that
nested repository. Commit only Rust-core integration or engine changes here.
Do not add the nested repository as ordinary tracked files or stage it with a
broad `git add -f`. It is intentionally not a submodule: a TextForge clone does
not automatically fetch the Python repository.

To obtain the binding beside the core:

```sh
git clone https://github.com/redlintles/textforge-python src/bindings/python
cd src/bindings/python
```

The Python repository builds independently from its versioned engine snapshot;
it does not require `../../..` to be an installed or published Rust dependency.
See its README and `docs/releasing.md` for development, tests and PyPI preparation.
Normal core Cargo commands no longer build a Python extension or depend on a
local Python interpreter.

The initial integration removes the two previously tracked binding scaffold
files from this repository's index while preserving their implementation in the
Python repository. No Rust public contract, core version or license is changed.
