# Debug output

`process_all_with_debug` prints every top-level step (`Step: [i] => [i+1]`,
the instruction, the text before and after). When a step runs other
instructions inside it, a nested trace follows the step:

```text
▶ cblk cleanup  (3 instructions)
  │ 1. tbs  16 → 14 chars
  │ 2. raw       14 → 12 chars
  │ 3. atb #   12 → 14 chars
◀ cleanup  16 → 14 chars (−2) · 2 → 2 lines

▶ rmape animals  (2 rules)
  │ rule 1 /cat/  'cat' → 'wolf'  @ 2..5
  │ rule 2 /wolf/  'wolf' → 'dog'  @ 7..11
◀ animals  14 → 14 chars (±0) · 2 → 2 lines · 2 replacements

▶ iter 2 times
  │ 1. ate 0  14 → 15 chars
  │ 2. ate 1  15 → 16 chars
◀ iter  14 → 16 chars (+2) · 2 → 2 lines

▶ onln 1  (line: 12 chars)
  │ 1. tua  12 → 12 chars
◀ onln  16 → 16 chars (±0) · 2 → 2 lines
```

- `▶` opens a container (`cblk`, `iter`, `onln`, `onck`, `onlni`, `rmape`) and
  `◀` closes it with its **net effect** on the text it received: characters
  (Unicode scalars) and logical lines before → after.
- `│ n.` is one nested step with the instruction as executed (variables and
  `[[ ]]` already resolved, e.g. `ate 0` for `ate '[[__COUNTER__]]'`) and the
  character count before → after. Nested containers open their own indented
  block instead of a step line.
- For `on*`, the header shows the size of the section; step counts describe the
  section, and the closing line describes the whole document.
- `rmape` lists each replacement with its rule (index, regex), the original and
  the new text, and the character range of the original match. At most 20
  replacements are listed per call, followed by `… +K replacements`. Snippets
  longer than 40 characters are shortened and control characters are escaped.
  The list comes from the single non-cascading pass that produced the result;
  nothing is applied again to describe it.

Guarantees:

- Debug output never changes the result: the trace only reads the documents.
  Tests compare debug and non-debug results for the same pipelines.
- The same trace is produced with and without the `buffered` feature. With
  `buffered`, it is written to the spooled log like the rest of the debug output,
  so no whole-log `String` exists.
- Outside debug runs the trace is disabled; each hook is one `Option` check.
- Colors use `colored`, like the rest of the debug output, and follow its
  enable/disable rules.

`process_single_with_debug` prints the same nested trace after its single step.
The instruction text in step lines uses each instruction's display form
(`to_textforge_line`), the same text shown in `Instruction:`; it is not quoted
for re-parsing.
