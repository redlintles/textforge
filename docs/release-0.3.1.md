# TextForge 0.3.1 core release

The core package version is **0.3.1**, published to crates.io on 2026-11-15.
It is a bug-fix release on top of 0.3.0, with no API removals. The
[changelog](../changelog.md) lists the behavior changes first; read them if
you match error texts or use `\n`/`\t` inside double quotes in `.textforge`
files.

## Scope

- Text pipelines: inside double quotes, `\n`, `\t`, `\r` and `\0` are now the
  control characters (fixes #1), so a literal line break or tab can be
  inserted from a `.textforge` file. Single quotes stay literal. See
  [text syntax](text-syntax.md).
- Round trip: parameters with line breaks are written back to `.textforge`
  with escapes instead of failing with a `SerializationError`.
- The text parser is `text::line_parser` (one scan, about 10% faster, see
  [benchmark](benchmarks/line-parser/)); `shell-words` is only a
  dev-dependency, used by the differential tests.
- Error messages are rewritten in English with the offending value and a
  suggested fix; `Display` is `error[N]: message`; the `instruction` field
  holds the source line. Error codes are unchanged.
- `txf` prints the result exactly, without prefix or added newline.
- Documentation: docs.rs front page, instruction index, English guides.
- No binding repository was modified.

## Validation

The release revision passed the full checklist (1,514 tests with
`test_access`) and rustdoc with `-D warnings`. `cargo +1.89 check` was not run
for this release because the toolchain was unavailable in the build
environment; run it in the `textforge-dev` container if you want that
confirmation for the declared minimum Rust version.
