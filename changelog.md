# Changelog

## 0.2.0 — Unreleased

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
