# Changelog

## Unreleased

### Added

- Execution profiles: `ExecutionProfile` (`balanced`, the previous behavior;
  `low-memory`; `fast`) with `spool_threshold`, set through
  `TextForgeProcessor::with_profile`/`set_profile` (and the portable
  `set_profile`). Profiles change memory use, never results; a test runs every
  processing method under every preset. `txf` reads a strict JSON `.txfrc`
  (schema in `docs/txfrc.schema.json`) and accepts `--profile`,
  `--spool-threshold`, `--config` and `--no-rc`; `txf config show` prints the
  effective profile and the source of each value. See
  [execution profiles](docs/execution-profiles.md).
- `ppjn {pipeline};` (pipeline join, opcode 0x58) runs a pipeline from another
  text or bytecode file on the current text, in a new execution context that
  shares only the static context; `end` inside it is local. The literal path
  is resolved against the containing file and loaded once with the outer
  pipeline; cycles and nesting deeper than 16 are errors, and errors name the
  joined file and line. Bytecode embeds joined pipelines through a new
  parameter type, 0x05 (protocol version unchanged), so it runs without the
  original files. Builder method `pipeline_join`; portable `InstructionSpec`.
  See [file instructions](docs/file-instructions.md).
- File instructions: `wtf {file};` writes the current text to a file and keeps
  it unchanged (atomic replace through a temporary file in the same directory;
  missing directories are an error), `apf {file};` appends a UTF-8 file and
  `inf {file} {index};` inserts one with exactly the index rules of `ins`.
  Paths accept interpolation. Opcodes 0x55–0x57; builder methods
  `write_to_file`, `append_file` and `insert_file`; available through the
  portable `InstructionSpec`. With `buffered`, `wtf` and `apf` work on spooled
  documents without loading them. See [file instructions](docs/file-instructions.md).
- Static context and built-in variables: `__PIPELINE_NAME__`,
  `__TEXTFORGE_VERSION__`, `__INPUT_FILE__` (empty for text input),
  `__LEN_CHARS__` and `__LINES__`, readable as `{{name}}`, in `[[ ]]`
  expressions and as numeric parameters (`onln '[[__LINES__]]' apply …`). The
  text-derived values are computed only when read and describe the text the
  instruction receives (the section, inside `on*`). `var`, `val`, `mutv` and
  `eval … in` reject these names with `ValidationError`, in text, bytecode and
  builders. New `TextForgeProcessor::set_pipeline_name` and `pipeline_name`
  (and their portable equivalents); a pipeline is named after its identifier,
  or after its file when read from one. See
  [execution context](docs/execution-context.md) and the
  [benchmark](docs/benchmarks/builtin-variables/README.md): about 16 ns more per
  run, no change for pipelines that do not read built-ins.
- Numeric parameters accept `[[ ]]` expressions and `{{ }}` templates, resolved
  at execution time: `onln '[[__COUNTER__ + 1]]' apply tbs;`. Text, bytecode,
  the builder and portable plans keep the expression. A result that is not a
  non-negative integer is an `IncompatibleTypeError`.
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

- `tucs` uppercased the wrong character (or none) when a multibyte character
  came before the index: it compared byte offsets with the character index.
- `slt` and `ctc` rejected an end index equal to the character count while
  accepting any larger one; both now clamp it, as documented.
- `cts` validated its word index against the character count (a missing word
  silently left the text unchanged); `cts` and `ctr` now report a word index
  past the last word.
- `cfw` skips leading whitespace, and `clw` finds the last word across any
  whitespace (it only split on `' '`, so `"ab\ncd"` capitalized `ab`) and
  ignores trailing whitespace.
- `padl` and `padr` with a huge length panicked (`capacity overflow`); they
  now return a `ValidationError`.
- `eval` and `rmp` wrote no parameters from `to_bytecode()`, so
  `TokenWrapper::to_bytecode_resolved` produced unreadable bytecode.
- Internal audit of `src/tokens`: `docs/audits/0.3.0-tokens-audit.md`.
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
- `end` inside a block or an iteration now stops the rest of that block or
  loop. Previously `cblk` and `iter` kept running the instructions after `end`
  (e.g. `ate x; end; ate y` in a block produced `xy`).
- The bytecode reader now applies the text parser's nesting limits. It used to
  reject valid three-level block payloads (`blk b assoc iter 2 times onln 1
  apply tbs;`) and accept two-level root nesting that text and builders reject.
- `end` now halts pipelines run with watchers (`process_all_with_watchers`).
  Watcher runs previously kept executing the instructions after `end`.

### Changed

- Dependencies: removed the unused optional test dependencies `rand` and
  `random-string` (and with them the unmaintained `instant`,
  RUSTSEC-2024-0384); updated `uuid` to 1.29.2 and `html-escape` to 0.3.1; ran
  `cargo update`, which moves `rustix` to 1.1.6 (fixes RUSTSEC-2026-0486).
- Internal audit of this work: `docs/audits/0.3.0-on-debug-audit.md`.
- Debug traces list at most 20 steps per container (`… +K steps`).
- Nested instructions (inside `iter`, `onln`/`onck`/`onlni`, conditionals and
  block payloads) with literal parameters are bound once per registered
  pipeline instead of on every execution; a nested regex is no longer
  recompiled each iteration (block `iter 2000 times onln 3 apply raw a A`:
  7.0 ms → 0.23 ms). `iter` and `on*` opt into preparation.
  `get_transform_vec` still returns templates without cached bindings.
- `[[ ]]` expressions and `eval` build their evaluation context from the
  variables the expression reads instead of copying every variable (with a
  1 MB variable present, 2000 evaluations: 89 ms → 4.8 ms).
- Variable references accept `_` in names, including as the first character:
  `{{__COUNTER__}}` and `{{line_no}}` now interpolate. Previously such markers
  did not match, their delimiters were dropped and the bare name was printed.
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

