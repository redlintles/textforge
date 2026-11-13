# File instructions

`wtf`, `apf` and `inf` let a pipeline write its current text to a file, or
read text from files, in the middle of processing. Their paths are **data**:
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
