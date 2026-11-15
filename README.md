# TextForge

[![crates.io](https://img.shields.io/crates/v/textforge.svg)](https://crates.io/crates/textforge)
[![docs.rs](https://img.shields.io/docsrs/textforge)](https://docs.rs/textforge)
[![License: GPL-3.0](https://img.shields.io/badge/License-GPL%203.0-blue.svg)](LICENSE)

A small language for **text-processing pipelines**, written in Rust. A pipeline
is a list of single-purpose instructions applied to a text in order: the output
of each step is the input of the next.

```text
"  hello world  "  →  tbs  →  rfw world Rust  →  tua  →  "HELLO RUST"
```

Write pipelines with a fluent Rust builder, in readable `.textforge` files, or
load them from compact `.textforgebc` bytecode, and run them on strings, files,
parallel batches or documents larger than memory.

> TextForge was previously called **ATP (Advanced Text Processor)**; some
> internal names still use the `atp` prefix.

## Contents

- [Highlights](#highlights)
- [Installation](#installation)
- [Quick start](#quick-start)
- [The language](#the-language)
- [Instruction reference](#instruction-reference)
- [Running pipelines](#running-pipelines)
- [Cargo features](#cargo-features)
- [Bindings](#bindings)
- [Documentation](#documentation)
- [Contributing](#contributing) · [License](#license)

## Highlights

- **89 instructions**: edits, deletions, regular-expression replacements, case
  and word transforms, encodings, Unicode normalization, logical lines and files.
- **Control flow**: variables and expressions, blocks, conditionals, loops,
  sections (`onln`, `onck`, `onlni`) and pipeline joins (`ppjn`).
- **Portable pipelines**: `.textforge` text files you can version and review,
  and `.textforgebc` bytecode that loads without parsing.
- **Large inputs**: batches in parallel (`rayon`), and buffered processing that
  spools documents to disk instead of holding them in memory.
- **Predictable**: each instruction does only what its name says; execution
  profiles change memory use, never results; safety limits stop runaway
  `rpt`, `padl`, `padr` and `iter`.
- **Observable**: a step-by-step debug trace and optional watchers that export
  a JSON report of a run.

## Installation

Library (Rust 1.89 or newer):

```toml
[dependencies]
textforge = "0.3.0"
```

Command-line program `txf`:

```sh
cargo install textforge --features cli           # add ,bytecode for .textforgebc files
```

## Quick start

### In Rust

```rust
use textforge::api::{
    TextForgeBuilderMethods,
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};

let mut processor = TextForgeProcessor::new();
let id = processor
    .create_pipeline()
    .trim_both_sides()?
    .replace_all_with("world", "Rust")?
    .to_uppercase_all()?
    .build()?;

assert_eq!(processor.process_all(&id, "  hello world  ")?, "HELLO RUST");
```

### In a `.textforge` file

```textforge
// clean.textforge — one instruction per line, each ending with ;
tbs;
rfw world Rust;
tua;
```

```rust
let id = processor.get_pipeline_from_file(std::path::Path::new("clean.textforge"))?;
let output = processor.process_all(&id, "  hello world  ")?;
```

### From the command line

```sh
txf --file clean.textforge --input input.txt --output output.txt
```

Without `--output` the result goes to stdout. `--profile`, `--spool-threshold`,
`--max-generated-bytes` and `--max-iterations` tune resources and limits; a
`.txfrc` file in the current directory can set them, and `txf config show`
prints the effective configuration. See
[execution profiles](docs/execution-profiles.md).

## The language

```textforge
// Comments take a whole line. Parameters are split like a shell command
// line: quote them when they contain spaces.
raw 'old name' 'new name';

// Inside double quotes, \n \t \r and \0 are control characters;
// single quotes are literal. See docs/text-syntax.md.
raw "\s+" "\n";

// {{name}} reads a variable or a built-in; [[ ... ]] evaluates an expression.
var who = 'world';
ate ', {{who}}!';
atb '[[__LINES__]] lines: ';

// Blocks collect instructions and run them later.
blk tidy assoc tbs;
blk tidy assoc dle;
cblk tidy;

// Conditionals, loops and sections (onln 1: only the first line).
ifdc 'TODO' do apl 'has todos';
iter 3 times ate '[[__COUNTER__]]';
onln 1 apply tua;
```

- Text pipelines use `.textforge` (alias `.tft`); bytecode uses `.textforgebc`
  (alias `.tfb`, `bytecode` feature).
- Characters are Unicode scalar values and indices start at 0; line numbers
  start at 1; `\n`, `\r\n` and `\r` all end a line and are preserved
  ([line semantics](docs/lines.md)).
- Built-in variables: `__PIPELINE_NAME__`, `__TEXTFORGE_VERSION__`,
  `__INPUT_FILE__`, `__LEN_CHARS__`, `__LINES__`, and `__COUNTER__` inside `iter`
  ([execution context](docs/execution-context.md)).

## Instruction reference

The same index, with links to each instruction's documentation, is on
[docs.rs](https://docs.rs/textforge/latest/textforge/tokens/index.html).

<!-- instruction-index:begin -->

### Add, insert and repeat

| Instruction | Syntax | Effect |
|---|---|---|
| `atb` | `atb {text};` | Add `text` to the beginning. |
| `ate` | `ate {text};` | Add `text` to the end. |
| `ins` | `ins {index} {text};` | Insert `text` after the character at `index`. |
| `padl` | `padl {text} {length};` | Pad on the left with `text` up to `length` characters. |
| `padr` | `padr {text} {length};` | Pad on the right with `text` up to `length` characters. |
| `rpt` | `rpt {times};` | Repeat the text `times` times. |
| `rev` | `rev;` | Reverse the characters. |
| `rtl` | `rtl {times};` | Rotate the characters `times` positions to the left. |
| `rtr` | `rtr {times};` | Rotate the characters `times` positions to the right. |
| `splc` | `splc;` | Separate every character with a space. |
| `rmws` | `rmws;` | Remove all whitespace. |

### Delete

| Instruction | Syntax | Effect |
|---|---|---|
| `dlf` | `dlf;` | Delete the first character. |
| `dll` | `dll;` | Delete the last character. |
| `dls` | `dls {index};` | Delete the character at `index`. |
| `dlc` | `dlc {start} {end};` | Delete the characters `start..=end`. |
| `dla` | `dla {index};` | Delete every character after `index`. |
| `dlb` | `dlb {index};` | Delete every character before `index`. |

### Replace and match (regular expressions)

| Instruction | Syntax | Effect |
|---|---|---|
| `raw` | `raw {pattern} {text};` | Replace every match of `pattern` with `text`. |
| `rfw` | `rfw {pattern} {text};` | Replace the first match. |
| `rlw` | `rlw {pattern} {text};` | Replace the last match. |
| `rnw` | `rnw {pattern} {text} {n};` | Replace the `n`th match. |
| `rcw` | `rcw {pattern} {text} {count};` | Replace the first `count` matches. |
| `rmp` | `rmp {pattern};` | Remove every match of `pattern`. |
| `emj` | `emj {pattern} {separator};` | Keep only the matches, joined with `separator`. |
| `rmape` | `rmape {map};` | Apply a replacement map defined with `rmapd`, without cascading. |

### Trim and select

| Instruction | Syntax | Effect |
|---|---|---|
| `tbs` | `tbs;` | Trim whitespace on both sides. |
| `tls` | `tls;` | Trim whitespace on the left. |
| `trs` | `trs;` | Trim whitespace on the right. |
| `slt` | `slt {start} {end};` | Keep only the characters `start..=end`. |
| `sslt` | `sslt {pattern} {index};` | Split by `pattern` and keep piece `index`. |

### Case

| Instruction | Syntax | Effect |
|---|---|---|
| `tua` | `tua;` | Uppercase everything. |
| `tla` | `tla;` | Lowercase everything. |
| `tucs` | `tucs {index};` | Uppercase the character at `index`. |
| `tlcs` | `tlcs {index};` | Lowercase the character at `index`. |
| `tucc` | `tucc {start} {end};` | Uppercase the characters `start..=end`. |
| `tlcc` | `tlcc {start} {end};` | Lowercase the characters `start..=end`. |
| `tucw` | `tucw {index};` | Uppercase word `index`. |
| `tlcw` | `tlcw {index};` | Lowercase word `index`. |
| `cfw` | `cfw;` | Capitalize the first word. |
| `clw` | `clw;` | Capitalize the last word. |
| `cts` | `cts {index};` | Capitalize word `index`. |
| `ctc` | `ctc {start} {end};` | Capitalize the words in the characters `start..=end`. |
| `ctr` | `ctr {start} {end};` | Capitalize words `start..=end`. |
| `jkbc` | `jkbc;` | Join the words as kebab-case. |
| `jsnc` | `jsnc;` | Join the words as snake_case. |
| `jcmc` | `jcmc;` | Join the words as camelCase. |
| `jpsc` | `jpsc;` | Join the words as PascalCase. |

### Encoding and Unicode

| Instruction | Syntax | Effect |
|---|---|---|
| `urle` | `urle;` | URL-encode. |
| `urld` | `urld;` | URL-decode. |
| `htmle` | `htmle;` | Escape HTML special characters. |
| `htmlu` | `htmlu;` | Unescape HTML entities. |
| `jsone` | `jsone;` | Escape as a JSON string. |
| `jsonu` | `jsonu;` | Unescape a JSON string. |
| `unorm` | `unorm {mode};` | Normalize Unicode to `nfc`, `nfd`, `nfkc` or `nfkd`. |

### Lines (line numbers start at 1)

| Instruction | Syntax | Effect |
|---|---|---|
| `apl` | `apl {text};` | Append a line. |
| `pln` | `pln {text};` | Prepend a line. |
| `inl` | `inl {line} {text};` | Insert a line before `line` (one past the last appends). |
| `rln` | `rln {line} {text};` | Replace the content of `line`, keeping its terminator. |
| `dln` | `dln {line};` | Delete `line`. |
| `dlna` | `dlna {line};` | Delete the lines after `line`. |
| `dlnb` | `dlnb {line};` | Delete the lines before `line`. |
| `dlni` | `dlni {start} {end};` | Delete lines `start..=end`. |
| `slnc` | `slnc {start} {end};` | Keep only lines `start..=end`. |
| `dle` | `dle;` | Delete empty lines. |
| `klm` | `klm {pattern};` | Keep the lines that match `pattern`. |
| `dlm` | `dlm {pattern};` | Delete the lines that match `pattern`. |
| `revln` | `revln;` | Reverse the order of the lines. |
| `uniqln` | `uniqln;` | Keep the first occurrence of each line. |
| `joinln` | `joinln {separator};` | Join the lines with `separator`. |
| `nln` | `nln;` | Remove every line terminator. |

### Files

| Instruction | Syntax | Effect |
|---|---|---|
| `wtf` | `wtf {file};` | Write the text to `file` and keep it unchanged. |
| `apf` | `apf {file};` | Append the contents of `file`. |
| `ppf` | `ppf {file};` | Prepend the contents of `file`. |
| `inf` | `inf {file} {index};` | Insert the contents of `file` after the character at `index`. |
| `ppjn` | `ppjn {pipeline};` | Run the pipeline in another file on the text. |

### Variables

| Instruction | Syntax | Effect |
|---|---|---|
| `var` | `var {name} = {value};` | Define a mutable variable. |
| `val` | `val {name} = {value};` | Define an immutable variable. |
| `mutv` | `mutv {name} = {value};` | Change a mutable variable. |
| `eval` | `eval {expression} in {name};` | Evaluate an expression into a variable. |

### Control flow

| Instruction | Syntax | Effect |
|---|---|---|
| `blk` | `blk {name} assoc {instruction};` | Add an instruction to block `name`. |
| `cblk` | `cblk {name};` | Run the instructions of block `name`. |
| `ifdc` | `ifdc {text} do {instruction};` | Run `instruction` if the text contains `text`. |
| `iter` | `iter {times} times {instruction};` | Run `instruction` `times` times (`__COUNTER__` counts from 0). |
| `onln` | `onln {line} apply {instruction};` | Run `instruction` on one line only. |
| `onck` | `onck {start} {end} apply {instruction};` | Run `instruction` on the characters `start..=end` only. |
| `onlni` | `onlni {start} {end} apply {instruction};` | Run `instruction` on lines `start..=end` as one section. |
| `rmapd` | `rmapd {map} add {pattern} {text};` | Add a rule to replacement map `map`. |
| `end` | `end;` | Stop the pipeline (or the current block or loop). |
| `null` | `null;` | Do nothing. |
<!-- instruction-index:end -->

## Running pipelines

| Method | Runs a pipeline on |
|---|---|
| `process_all` | a string |
| `process_file` | a file, writing the result to another file |
| `process_batch` | many files in parallel |
| `process_*_buffered` (`buffered`) | documents spooled to disk above a threshold |
| `process_all_with_debug` | a string, returning a step-by-step trace ([debug output](docs/debug-output.md)) |
| `process_all_with_watchers` (`watchers`) | a string, collecting a JSON report |

An `ExecutionProfile` (`balanced`, `low-memory`, `fast`) decides how much memory
a run may use and the safety limits of work-multiplying instructions; it never
changes the output. Pipelines can be saved and loaded as text
(`write_pipeline_to_file`, `get_pipeline_from_file`) or bytecode
(`write_to_bytecode_file`, `read_from_bytecode_file`).

## Cargo features

| Feature | Enables |
|---|---|
| `bytecode` | Reading and writing `.textforgebc` pipelines |
| `watchers` | Watchers and JSON execution reports |
| `buffered` | Buffered processing (`process_*_buffered`, `TextBuffer`); pulls in `tempfile` |
| `cli` | The `txf` program; pulls in `clap` |

No feature is enabled by default. `test_access` exposes internals for the test
suite and is not meant for users.

## Bindings

Bindings are independent projects with their own versions, built on the
[portable API](docs/portable-api.md):

- Python: [`textforgers`](https://github.com/redlintles/textforge-python) ([notes](docs/python-bindings.md))
- Node.js: [`textforge-node`](https://github.com/redlintles/textforge-node) ([notes](docs/node-bindings.md))

## Documentation

- [API documentation on docs.rs](https://docs.rs/textforge): every type,
  method and instruction, with examples.
- [Guides and design notes](docs/README.md): file and section instructions,
  replacement maps, profiles, buffering, the portable API, benchmarks and audits.
- [Changelog](changelog.md), with upgrade notes for each release.

## Contributing

Contributions are welcome. Code, comments, commits and documentation are written
in English, and commits follow
[Conventional Commits](https://www.conventionalcommits.org/)
(`type(scope): description`). [`AGENTS.md`](AGENTS.md) describes the
architecture, the rules every change follows and the pre-commit checklist;
adding an instruction is described in its section 12.

## License

[GPL-3.0](LICENSE): derived works must remain open source.
