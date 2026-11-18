# TextForge 0.4.0 core release

The core package version is **0.4.0**. It is **prepared, not yet
published**: publishing to crates.io is the maintainer's decision
(`AGENTS.md`, section 15). The latest published version is 0.3.2.

0.4.0 is about robustness and diagnostics. Read the **Breaking changes** and
**Behavior changes** of the [changelog](../changelog.md) before upgrading.

## Scope

- **`tryi {instruction};`** skips an instruction that fails and leaves no
  trace of it: the text, the variables, the blocks, the replacement maps and
  `end` are as they were before it. Safety limits are never ignored. The debug
  methods and `txf --debug` show every ignored error. Builder `try_ignore`,
  portable mnemonic `tryi`, opcode `0x5a`.
- **Error reports.** Errors raised while a pipeline runs carry the trace of the
  instructions they left (with `file:line`), the parameters of the instruction
  that failed with the values it ran with, and the cause. `Display` is one
  plain line; `TextForgeError::report()` is a colored traceback, which `txf`
  prints. Each code has a stable name and a hint.
- **`LimitExceededError` (31)** for `max_generated_bytes` and `max_iterations`,
  which were `ValidationError` (20).
- **Portable API**: `BindingError::details`.
- The processor no longer keeps a copy of every error (`ErrorManager` is
  deprecated).
- **`txf` fixes** of the 0.3.2 audit ([report](audits/0.3.2-cli-audit.md)):
  read modes and line terminators, exit statuses 0–4, `--mode auto`,
  `compile`/`decompile`/`check`, `--watch`, `-` for stdin/stdout, atomic
  `--output`, a structured `--help` and the [guide](cli.md).
- No binding repository was modified. The bindings are expected to move from
  0.3.1 straight to 0.4.0; the [parity matrix](portable-parity.md) lists what
  they need (the 0.3.2 index change, the `txf` changes, `tryi` and the error
  details).

## Validation

The release revision passed the full pre-commit checklist of `AGENTS.md`
(section 14) with every feature set, and the benchmarks of
[`tryi` and error details](benchmarks/tryi/README.md) and
[the command line](benchmarks/cli/README.md).

## Publishing

```sh
cargo package --list          # only src/, the schema, README, changelog, LICENSE
cargo publish --dry-run
git tag v0.4.0 && git push origin main v0.4.0
cargo publish
```

Then change the `Unreleased` heading of the changelog to `0.4.0 (date)` and
the published version in `AGENTS.md`.
