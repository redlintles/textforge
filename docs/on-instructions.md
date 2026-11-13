# Section instructions: `onln`, `onck`, `onlni`

The `on*` instructions apply **one** instruction to part of the text. The result
replaces that part; the rest of the document is unchanged.

| DSL | Rust builder | Section | Index base |
| --- | --- | --- | --- |
| `onln {n} apply {instruction};` | `on_line(n, inner)` | line `n` | **1**, like `dln`, `slnc`, `dlni` |
| `onck {start} {end} apply {instruction};` | `on_chunk(start, end, inner)` | characters `start..=end` (Unicode scalars) | **0**, like `dlc`, `slt` |
| `onlni {start} {end} apply {instruction};` | `on_line_interval(start, end, inner)` | lines `start..=end`, as **one** block | **1** |

> Line numbers start at **1**; character indices start at **0**. Both ranges
> include their end.

`apply` is a syntax-only literal, dropped during parsing like `assoc` and `do`.
Opcodes: `onln` 0x52, `onck` 0x53, `onlni` 0x54.

## Semantics

1. **The nested instruction receives only the section.** Every index it uses is
   relative to the section: `onln 3 apply dlf;` deletes the first character of
   line 3, not of the document. For every index-based instruction, `on* … apply X`
   produces exactly what extracting the section, applying `X` to it alone and
   splicing it back produces (`tests/on_instructions.rs` checks this for the
   index-based transforms and the line family).
2. **Terminators.** `onln`/`onlni` pass the line (or block) **without** the
   terminator of the last selected line; that terminator stays outside and is
   preserved. Terminators between lines of an `onlni` block stay inside the
   block. All terminators of [logical lines](lines.md) are recognized.
3. **Any number of lines.** The result may have more lines, fewer, or none:
   `onlni 10 20 apply uniqln;` and `onlni 2 50 apply dle;` change the line count.
   The no-line-break restriction of `inl`/`rln` does not apply here.
4. **`onln n` equals `onlni n n`.** Both use the same implementation
   (`src/utils/sections.rs`).
5. **Context.** The nested instruction shares the pipeline's execution context:
   it can read variables and call blocks, and `end` inside it halts the pipeline
   after the `on*` instruction finishes.

Because the section excludes the final terminator, a filter that removes the last
selected line leaves the terminator before it in place, exactly as the filter
does on the extracted block by itself:

```text
input: "b\na\nb\na\nc"    pipeline: onlni 1 4 apply uniqln;
block: "b\na\nb\na"        uniqln → "b\na\n"
output: "b\na\n\nc"
```

## Errors

All errors are `TextForgeError`; nothing panics.

| Instruction | Error | Same as |
| --- | --- | --- |
| `onln`, `onlni` | line `0`, line past the end, reversed interval, empty document (zero lines) | `dlni`/`slnc` (identical error values) |
| `onck` | `start > end` (`InvalidIndex`), `start` outside the text (`IndexOutOfRange`), empty document | `dlc`, with the documented difference below |
| all | an error from the nested instruction | — |

`start == end` selects one character (`onck 0 0 apply tua;`), as in every
inclusive range, and an empty document is an error, as in `dlc`.

As in `dlc`, an `end` past the last character is clamped to it.

## Structural rules

`on*` inherit the structural rules of `iter` (see
[builder validation](builder-validation.md) and
[validation rules](validation-rules.md)):

- `rmapd` cannot be placed inside them (directly or deeper).
- The nesting depth limits are the parser's: the root allows **one** nested
  level, a block payload (`blk … assoc …`) allows three. So
  `onln 1 apply tbs;` and `blk b assoc iter 3 times onln 1 apply tbs;` are valid,
  but `iter 3 times onln 1 apply tbs;` at the root and an `on*` inside an
  `ifdc` body at the root are rejected with `Nested token depth exceeded`.

## Applying to each line

Combining `iter` and `onln` reaches lines one by one: iteration `i` sees
`__COUNTER__ == i`, and numeric parameters accept `[[ ]]` expressions, so the
line number is `[[__COUNTER__ + 1]]`. Because of the depth limit above, the
combination runs from a block:

```text
blk each assoc iter 3 times onln '[[__COUNTER__ + 1]]' apply tbs;
cblk each;
```

Quote expressions that contain spaces, as with any parameter.

If the nested instruction adds or removes lines, the following line numbers
shift. Use `onlni` when the line count changes.

## Debugging

Debug runs show the section size and the nested step for every `on*` call; see
[debug output](debug-output.md).

## Performance

- Lines are located with one scan for terminators that stops at the last selected
  line and allocates nothing; characters with one `char_indices` scan (O(n) in
  the position of `end`).
- The section is a borrowed slice of the input. When the nested result equals the
  section, the input is returned without copying. Otherwise a borrowed input is
  assembled once into a new buffer, and an owned input is edited in place.
- Buffered execution uses the existing whole-document fallback; there is no
  native spool implementation yet. See the
  [benchmark](benchmarks/on-instructions/README.md).
