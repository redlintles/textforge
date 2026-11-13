# File instructions

`ppjn` runs a pipeline from another file. `wtf`, `apf` and `inf` let a
pipeline write its current text to a file, or read text from files, in the
middle of processing.

## `ppjn` (pipeline join)

`ppjn {pipeline};` runs a whole pipeline from another file on the current text
and continues with its result. It works like a subshell: text in, text out.

```text
// main.textforge
ppjn lib/clean.textforge;     // lib/ is next to main.textforge
ate ' — {{__PIPELINE_NAME__}}';
```

| Topic | Rule |
| --- | --- |
| Path | A **literal** (no `{{ }}`/`[[ ]]`: `ValidationError`). Relative to the file containing `ppjn`; for pipelines built in code (builder, portable plans, `add_transform`), relative to the working directory |
| Loading | When the outer pipeline is loaded (`get_pipeline_from_file`, `build`, portable `register`, `add_transform`), never while it runs: `iter 1000 times ppjn lib.textforge;` reads the file once. A file that fails to load fails `get_pipeline_from_file`, `build` and `register`; `add_transform` cannot report errors, so there the join loads, and reports, when it runs |
| Format | Text or bytecode, recognized by the first bytes (the bytecode magic number), not the extension. Bytecode without the `bytecode` feature is an error |
| State | The joined pipeline runs in a **new** execution context: outer variables, blocks and replacement maps are invisible, and its own do not leak out. It shares only the static context (`__PIPELINE_NAME__` stays the outer name; `__LINES__` and `__LEN_CHARS__` describe the text it receives) |
| `end` | Ends only the joined pipeline |
| Cycles | Detected when loading, with the whole chain in the error input (`a.textforge → b.textforge → a.textforge`). Nesting is limited to `MAX_JOIN_DEPTH` (16) |
| Modes | The joined pipeline runs in the outer mode: plain, debug (its steps appear nested under `▶ ppjn {path}`), watchers (one outer step) and buffered (spooled text stays spooled) |
| Errors | Name the file and line (`ppjn lib.textforge:3 › dlc 99 100;`); the file contents are never quoted |
| Bytecode | The outer bytecode **embeds** the joined pipeline (parameter type 0x05, see `AGENTS.md` §8), so it runs without the original files. Converting back to text writes `ppjn "path";` |

Builder: `pipeline_join(path)`. Portable: `InstructionSpec` `ppjn` with one
`Argument::Text`. Opcode 0x58.

## Reading and writing files

The paths of `wtf`, `apf` and `inf` are **data**:
they may use `{{ }}` and `[[ ]]` interpolation and are resolved when the
instruction runs. A relative path is relative to the process working
directory. Files are read with the same helper as `process_file`
(`read_utf8_file`): no `mmap`, no `unsafe`, and opening, reading and invalid
UTF-8 are distinct `TextForgeError`s that name the path.

| Instruction | Syntax | Effect | Builder |
| --- | --- | --- | --- |
| `wtf` | `wtf {file};` | Write the current text to `file`; the text is unchanged | `write_to_file(path)` |
| `apf` | `apf {file};` | Append the contents of `file` to the end of the text | `append_file(path)` |
| `inf` | `inf {file} {index};` | Insert the contents of `file` after the character at `index` (base 0) | `insert_file(path, index)` |

Opcodes: `wtf` 0x55, `apf` 0x56, `inf` 0x57. Portable plans use the same
mnemonics with `Argument::Text` paths and an `Argument::Number` index.

```text
tbs;
wtf 'out/{{__PIPELINE_NAME__}}-trimmed.txt';   // save an intermediate state
apf 'templates/footer.txt';
inf 'templates/signature.txt' 0;
```

## `wtf` (write to file)

- Creates the file or replaces it. A missing directory is an error
  (`FileNotFound`); directories are never created.
- **Atomic:** the text is written to a temporary file in the same directory
  (`.{name}.{pid}-{n}.tmp`), flushed to disk, then renamed over the target. If
  any step fails, the previous file is untouched and the temporary file is
  removed. This is stronger than the non-atomic output of `process_file`
  ([safe file I/O](safe-file-io.md)).
- Replacing a file gives it default permissions; a symbolic link at the path is
  replaced, not followed.
- Returns the text unchanged and without copying it. With `buffered`, a
  spooled document is streamed to the file without loading it into memory.
- In `process_batch`, tasks should write to independent paths (for example
  `wtf 'out/{{__INPUT_FILE__}}.snapshot';`). Two tasks writing the same path
  never interleave: the last rename wins.
- Debug output adds a line with the number of bytes written and the path.
  Watchers see it as an ordinary step that does not change the text.

## `apf` (append file)

Reads the whole file and appends it. An empty file returns the text without
copying it; otherwise the text's own buffer grows when it is owned. With
`buffered`, a spooled document stays on disk. The file is read every time the
instruction runs (inside `iter 100 times`, a hundred times).

## `inf` (insert file)

Uses **exactly** the index rules of `ins` (the two share one function): the
contents go after the character at `index`, counting Unicode scalar values from
0; an index equal to the character count appends; a larger index is an
`IndexOutOfRange` error. Like `ins`, it cannot insert before the first
character; use `apf` for the end. The file is read before the index is
checked, so a file error is reported first.

Inside `onln`, `onck` and `onlni`, `inf`'s index is relative to the section,
and `wtf` writes only the section.
