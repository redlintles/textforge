# Changelog

## Unreleased

### Behavior changes

- Text pipelines: spaces inside an unquoted expression no longer split it, so
  `onln [[__COUNTER__ + 1]] apply tbs;` works without quotes (before: the line
  was split at each space and failed). A `[[` with no closing `]]` later on
  the line is still ordinary text; variable templates (`{{name}}`) take no
  spaces and are unchanged. See
  `docs/text-syntax.md`.

### Added

- `txfg`, the TextForge game (`game` feature,
  `cargo install textforge --features game`): matches of rounds, each a JSON
  game file with a start text, a goal text, the allowed instructions, a
  pipeline size limit and a reference answer. The player builds the pipeline
  at a prompt with `!what`, `!undo`, `!reset`, `!show_answer` and other
  commands; rounds score 0-100 from the instructions used and the time taken.
  Thirty built-in games, difficulty selection (`-d`), round count (`-r`), game
  directories (`--pool`), validation and a 0-10 difficulty rating
  (`--check`, `--list`) and JSON results (`--save-result`). See `docs/txfg.md`.
- `scripts/instruction_index.py` also writes the game's instruction catalog
  (`src/bin/txfg/catalog.rs`); `tests/instruction_index.rs` checks it.

## 0.3.1 (2026-11-15)

### Behavior changes

- Error messages are rewritten in English and describe what went wrong, the
  offending value and, when there is one, how to fix it (for example
  "parameter 1 (`index`) must be a non-negative integer, but it is text"
  instead of "Index should be of usize type"). Code that matches error texts
  must be updated; error codes are unchanged.
- `TextForgeError`'s `Display` is now `error[N]: message`, followed by
  `in: <instruction>` and `input: <input>` lines when those fields are set.
- In a pipeline, an error's `instruction` field holds the source line of the
  failing instruction (for example `ctc 1 8;`). Internal function names are no
  longer used as labels; errors from the engine say `processor`, `txf` or
  `txfrc`. Errors from `transform()` called directly leave the field empty.
- `txf` prints the result exactly, without the former "Resultado do
  processamento:" prefix and without an added newline, so its output can be
  piped. Its help texts and read-mode errors are in English.
- Text pipelines: inside double quotes, `\n`, `\t`, `\r` and `\0` are now the
  newline, tab, carriage return and NUL characters, so `atb "a\nb";` inserts a
  line break (before: a backslash and a letter). Single quotes stay literal and
  every other escape keeps its meaning (`"\d"` still reaches a regex as `\d`).
  Fixes #1. See `docs/text-syntax.md`.
- `\n`, `\t`, `\r` and `\0` outside quotes, and a real line break inside an
  instruction passed to `read_from_text`, are `TextParsingError`s instead of
  being read as plain letters.

### Fixed

- Pipelines whose text parameters contain line breaks can be written back to
  `.textforge` (`write_to_file`, bytecode to text): they are quoted with
  escapes instead of failing with a `SerializationError`.

### Changed

- Documentation: the crate has a front page on docs.rs (overview, examples,
  where to start, concepts, features) and every public module and item is
  documented in English; an instruction index on the `tokens` page links every
  mnemonic to its struct; feature-gated items carry feature badges on docs.rs.
  Engine plumbing that users cannot call meaningfully (`utils::apply`, `expr`,
  `regexes`, `transforms`, `validations`, `bytecode::reader`/`writer`) is hidden
  from the index; it remains public.
- The README is reorganized (installation, quick start, language, the complete
  instruction reference, processing modes, features), and `docs/README.md` is
  an index of the guides, design notes, benchmarks and audits, now all in English.
- The text parser no longer uses `shell-words`: lines are split by
  `text::line_parser` in one scan, about 10% faster
  (`docs/benchmarks/line-parser`). `shell-words` is now only a dev-dependency,
  used by the differential tests.
- Only `txf` is built as a binary: the empty placeholders in `src/bin` are no
  longer discovered (`autobins = false`).

## 0.3.0 (2026-11-14)

Upgrade notes: the breaking changes are listed first, then the behavior
changes an existing pipeline may notice. Everything else is additive.

### Breaking changes

- The `txf` command-line program and `utils::cli` are behind the new `cli`
  feature, so the library no longer depends on `clap`. Install the program with
  `cargo install textforge --features cli` (add `bytecode` for `.textforgebc`).
- `TextForgeBuilderMethods::define_replace_map(name, configure)` takes a closure
  that fills a scoped `ReplaceMapBuilder` (`map.add_replace(a, b)?.end()`)
  instead of one `(name, pattern, replacement)` rule. One-rule definitions use
  `add_replace_rule(name, pattern, replacement)`.
- `GlobalContextMethods::get_all_vars` was removed: expressions now read only
  the variables they use. Read single variables with `get_var`.
- `SyntaxToken` and `TextForgeParamTypes` have a new `Pipeline` variant (the
  pipeline joined by `ppjn`) and are now `#[non_exhaustive]`: matches on them
  need a wildcard arm, and future variants will not be breaking.
- The Cargo features `rand` and `random-string` were removed (they only enabled
  unused test dependencies).
- Minimum supported Rust version: 1.89 (`rust-version`).

### Behavior changes

- Safety limits: `rpt`, `padl` and `padr` fail above `max_generated_bytes`
  (2 GiB) and `iter` above `max_iterations` (10,000,000), instead of running
  out of memory. Both are configurable or `"unlimited"` (see Added).
- `tucw`, `tlcw`, `cts` and `ctr` keep the original separators between words
  (they used to rejoin the text with single spaces, joining lines).
- Inclusive character and word ranges accept `start == end`; `tucc`/`tlcc`
  clamp an end past the text; `dlc` on empty text is an `IndexOutOfRange` error.
- `end` inside a block or an iteration stops the rest of that block or loop,
  and halts runs with watchers.
- `iter` iteration `i` sees `__COUNTER__ == i` (it saw `0, 0, 1, …`).
- `{{__COUNTER__}}`, `{{line_no}}` and other names with `_` interpolate (their
  delimiters used to be dropped and the bare name printed).
- `var`, `val`, `mutv` and `eval … in` reject the five built-in names
  (`__PIPELINE_NAME__`, `__TEXTFORGE_VERSION__`, `__INPUT_FILE__`,
  `__LEN_CHARS__`, `__LINES__`).
- Bytecode files follow the text parser's nesting limits.
- Index and parsing errors keep an excerpt of the processed text in their
  `input` field instead of the whole document (see Fixed).

### Added

- `ppf {file};` (prepend file, opcode 0x59) puts the contents of a UTF-8 file at
  the beginning of the text, the counterpart of `apf`. Builder method
  `prepend_file`; portable `InstructionSpec` `ppf`. With `buffered`, spooled
  documents stay on disk. See [file instructions](docs/file-instructions.md).
- `cli` feature (see Breaking changes) and complete package metadata:
  `rust-version`, keywords, categories and docs.rs features.
- `utils::errors::input_excerpt` and `INPUT_EXCERPT_CHARS`, the excerpt errors
  use for large texts.
- Safety limits for instructions that multiply work: `rpt`, `padl` and `padr`
  fail when their output would exceed `max_generated_bytes` (default 2 GiB),
  and `iter` when its count exceeds `max_iterations` (default 10,000,000),
  instead of exhausting memory, CPU or (buffered `rpt`) the disk. Set them in
  `ExecutionProfile::limits`, in `.txfrc` `overrides` (including
  `"unlimited"`) or with `txf --max-generated-bytes`/`--max-iterations`.
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

- Index errors (lines, sections, characters, words, `ins`/`inf`, `dls`, `dla`,
  `dlb`, `dlc`, `sslt`) and the parsing errors of `jsone`, `jsonu` and `urld`
  copied the whole processed text into the error, and some into the message: on
  a 100 MB document one error used 100 MB more. They now keep up to 80
  characters followed by `… [N bytes]`; shorter texts appear whole, as before.
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
- `tucw`, `tlcw`, `cts` and `ctr` change only the chosen words and keep the
  original separators; they used to rebuild the text with single spaces,
  joining lines.
- Inclusive character and word ranges accept `start == end` (one item), as
  `onck` did. `tucc` and `tlcc` clamp an end past the text like `slt`, `ctc`
  and `dlc`; `dlc` on empty text is an index error; a start past the text is
  reported as `IndexOutOfRange` in every range instruction.
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

- The published package holds only the library sources, the `.txfrc` schema,
  README, changelog and license (`include` in `Cargo.toml`): 1.4 MiB instead of
  204.8 MiB of benchmark data, logs and scripts.
- Engine performance: parameter templates and `[[ ]]` expressions are compiled
  once per registered pipeline (constant expressions are evaluated once),
  expressions are evaluated against a small read-only context, the context
  instructions (`var`, `val`, `mutv`, `eval`, `blk`, `cblk`, `ifdc`) are bound
  once, instructions with dynamic parameters reuse their last binding while the
  values repeat (a `raw {{p}} X` regex is no longer recompiled on each run),
  and bytecode files are decoded from memory. Output and errors are unchanged.
  Constant expression parameters run 64× faster, templates 7×, blocks 6×,
  variable-heavy pipelines 5×, `iter` with an expression 3.9× and bytecode
  loading 2.1×; see `docs/benchmarks/engine/README.md`.
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

