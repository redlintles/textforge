# Changelog

## 2.0.0 — Unreleased

### Performance and compatibility

- Preserve borrowed slices for trims and selections, reuse owned storage for
  localized edits, and remove temporary strings per character/word and regex
  match. Preserve unchanged input in regex, HTML and control-flow paths.
- Avoid temporary line-instruction construction and redundant regex-pattern/map
  name clones. Preserve Unicode contextual casing, line endings, existing error
  behavior, public APIs and big-endian bytecode opcodes.
- Add an exact 2,106-case baseline parity corpus covering all 81 instructions,
  borrowed/owned storage regression tests, and reproducible large-file benchmarks
  recording wall time, CPU time/utilization and peak resident memory.
- Prepare the requested 2.0.0 core version. The version jump does not introduce an
  intentional API or instruction-result break relative to core revision 72fc7fc.
  Existing index edge cases remain documented; independent language bindings are
  not repackaged or published as part of this core release.

See [allocation optimization and measurements](docs/allocation-optimization.md)
and [release validation](docs/release-2.0.0.md).


### Added

- Unicode normalization with `unorm`: NFC, NFD, NFKC and NFKD, available through
  the DSL, fluent Rust builder and portable API. Includes official Unicode
  conformance tests and prepared execution support.
- Named regex replacement maps: `rmapd` defines context-owned maps and `rmape`
  applies replacements without cascading across newly inserted text.
- Sixteen logical-line transforms, with line numbers starting at 1:
  - `apl`, `pln`, `inl`, `rln`: append, prepend, insert and replace one line.
  - `dln`, `dlna`, `dlnb`, `dlni`: delete one line, lines after/before a retained
    boundary, or an inclusive interval.
  - `slnc`: select an inclusive line interval.
  - `dle`: remove exactly empty lines, retaining whitespace-only lines.
  - `klm`, `dlm`: keep/delete lines matching a regular expression.
  - `revln`, `uniqln`: reverse line contents and retain first unique occurrences.
  - `nln`, `joinln`: remove line terminators or join contents with a separator.
- Matching fluent Rust APIs and portable registry support for every new token,
  including variables, blocks, conditionals, prepared execution, watchers and
  buffered whole-document execution. Text and big-endian bytecode round trips
  are covered by integration tests.
- Typed Python adapters and parity fixtures in the independent binding
  repository, targeting binding version **0.1.2** with core **0.2.0**.

### Line semantics

- Empty input has zero lines. A trailing terminator belongs to the preceding
  line; it does not create a phantom final line.
- LF, atomic CRLF, CR, VT, FF, NEL, LINE SEPARATOR and PARAGRAPH SEPARATOR are
  recognized. Surviving lines retain their original terminators.
- Invalid, zero or reversed line indices produce structured errors. Insertion
  also accepts line count + 1. Inserted and replacement payloads contain one
  logical line; `joinln` accepts arbitrary separator text.
- Line insertion follows the first existing terminator, falling back to LF.
  Reversal retains terminators by position; a blank line moved to an
  unterminated EOF receives a terminator so that it remains a real line.

See [logical line operations](docs/lines.md),
[Unicode normalization](docs/unicode-normalization.md), and
[regex replacement maps](docs/replace-maps.md).

This entry prepares the release; it does not indicate publication to a registry.
