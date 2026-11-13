# Changelog

## Unreleased

### Added

- Nested debug trace: `process_all_with_debug` and `process_single_with_debug`
  now show what happens inside `cblk`, `iter`, `onln`/`onck`/`onlni` and `rmape`
  (steps, net effect in characters and lines, and up to 20 replacements per
  `rmape` call), with and without `buffered`. Debug output never changes the
  result. See [debug output](docs/debug-output.md).
- Section instructions `onln` (one line, base 1), `onck` (character chunk,
  base 0) and `onlni` (line interval as one block, base 1):
  `onln {n} apply {instruction};`. The nested instruction receives only the
  section, so its indices are relative to it. Opcodes 0x52–0x54; builder methods
  `on_line`, `on_chunk` and `on_line_interval`; available through the portable
  `InstructionSpec`. See [section instructions](docs/on-instructions.md) and the
  [benchmark](docs/benchmarks/on-instructions/README.md).
- `ReplaceMapBuilder`: a scoped fluent builder for replacement maps
  (`define_replace_map(...)`, then `add_replace(...)` for each rule, then `end()`).
  See [replacement maps](docs/replace-maps.md) and [portable API](docs/portable-api.md).

### Fixed

- Block binding during debug execution (`process_all_with_debug` and related
  debug paths).
- `iter`: iteration `i` now sees `__COUNTER__ == i`. The body previously saw
  `0, 0, 1, …, times-2` because the counter was updated after each iteration.
- `iter` nested inside `iter`: the inner loop hides the outer counter and
  restores it when it ends. Previously the inner loop deleted the shared
  variable and the outer loop failed with `VariableNotFound`.
- `iter` without an execution context returns `RequiredContextError` instead of
  panicking, and `iter` bytecode now includes its two parameters.
- `ctc`, `ins` and `slt` no longer overflow (panicking in debug builds) on
  empty input; they return the error release builds already returned.
- `end` now halts pipelines run with watchers (`process_all_with_watchers`).
  Watcher runs previously kept executing the instructions after `end`.

### Changed

- Every processing mode (plain, debug, watchers and their buffered variants)
  now runs the same internal execution loop, so halting and ordering rules
  cannot drift between modes. Public signatures are unchanged.
- The Node.js binding (`src/bindings/node`) is an independent repository, excluded
  from the Cargo workspace and ignored by Git, like the Python binding. See
  [Node.js binding repository](docs/node-bindings.md).
- `AGENTS.md` now requires `AGENTS.md`, `README.md` and the changelog to be
  updated with every substantial change (section 18). Stale facts were corrected:
  core version, file extensions, Cargo features, dependencies and repository layout.
- The release notes document is now `docs/release-0.2.0.md`. Its old name used
  the abandoned `2.0.0` version.

## 0.2.0

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
- Prepare the requested 0.2.0 core version. The version jump does not introduce an
  intentional API or instruction-result break relative to core revision 72fc7fc.
  Existing index edge cases remain documented; independent language bindings are
  not repackaged or published as part of this core release.

See [allocation optimization and measurements](docs/allocation-optimization.md)
and [release validation](docs/release-0.2.0.md).


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

