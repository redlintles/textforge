# Indices, ranges and replacement text

One page with the rules that every instruction with a position follows, and the
exceptions that exist for compatibility.

## Bases

| What is counted | First value | Instructions |
|---|---|---|
| Characters (Unicode scalar values) | **0** | `dls`, `dlc`, `dla`, `dlb`, `slt`, `ins`, `inf`, `tucs`, `tlcs`, `tucc`, `tlcc`, `ctc`, `onck` |
| Words (separated by whitespace) | **0** | `tucw`, `tlcw`, `cts`, `ctr` |
| Matches and pieces | **0** | `rnw`, `sslt` |
| Lines | **1** | `dln`, `dlna`, `dlnb`, `dlni`, `slnc`, `inl`, `rln`, `onln`, `onlni` |

Ranges include both ends (`dlc 1 3` deletes three characters) and
`start == end` selects one item.

## Out of range

| Case | Result | Examples |
|---|---|---|
| A single index that does not exist | error (`IndexOutOfRange`) | `dls 9`, `dla 9`, `tucs 9`, `cts 9`, `sslt , 9`, `dln 9`, `onln 9` |
| The **start** of a range does not exist | error | `dlc 9 12`, `slnc 9 12` |
| The **end** of a range is past the text | the end is the last item | `dlc 1 99`, `slt 1 99`, `tucc 1 99`, `ctr 1 99`, `onck 1 99`, `dlni 2 99`, `slnc 2 99`, `onlni 2 99` |
| `start > end` | error (`InvalidIndex` for characters and words, `InvalidParameters` for lines) | `dlc 3 1`, `dlni 3 1` |
| Line `0` | error | `dln 0`, `slnc 0 2` |
| Empty text where a position is needed | error | `dlc 0 3` on empty text |

Line ranges clamp their end since 0.3.2; before, `dlni 2 99` was an error
while `dlc 2 99` was not.

### Replacing never fails for lack of a match

`raw`, `rfw`, `rlw`, `rnw`, `rcw` and `rmapd` leave the text unchanged when
nothing matches, and `rnw {pattern} {text} {n}` does the same when there is no
match number `n`: `n` counts matches, not positions in the text, so it is not
an index in the sense of the table above.

### Exceptions

- **`ins {index} {text}`** and **`inf {file} {index}`**: index `0` inserts at
  the start; any other index inserts *after* the character at that position.
  For a text of `n` characters, `index` may be `0..=n`; both `n - 1` and `n`
  append. The position between the first and second character has no index of
  its own (`ins 0` is before the first character, `ins 1` is after the second);
  reach it with `rfw` and a pattern, or `onck 0 0 apply ate ...`. Up to 0.3.1,
  `0` inserted after the first character and the start was unreachable.
- **`inl {line} {text}`** accepts the line count plus one, which appends.

## Argument order

The position comes where the phrase the name abbreviates puts it, which is not
the same place in every instruction:

| Instruction | Order |
|---|---|
| `ins`, `inl`, `rln` | position, then text |
| `inf` | file, then position |
| `rnw`, `rcw`, `sslt`, `padl`, `padr` | pattern or text, then number |

## Patterns are regular expressions

`raw`, `rfw`, `rlw`, `rnw`, `rcw`, `rmp`, `emj`, `sslt`, `klm`, `dlm` and
`rmapd` take a [regex](https://docs.rs/regex/latest/regex/#syntax) pattern, so
`.`, `+`, `(`, `$` and the other metacharacters must be escaped to match
themselves: `raw '1\+1' 2;`, `sslt '\.' 0;`. Look-around and backreferences
are not supported by the regex engine. `ifdc` is the exception: its parameter
is plain text, and the test is "the text contains it".

Lines that end with `\r\n`: `$` in multi-line mode matches before `\n` only,
so `(?m)x$` does not match `x\r\n`. Use `(?mR)x$` (CRLF mode), or a line
instruction (`klm`, `dlm`), which never see the terminator.

## `$` in replacement text

The replacement of `raw`, `rfw` and `rcw` is a regex replacement template:
`$1`, `$name` and `${name}` insert capture groups. **`rlw` and `rnw` do not
expand it**: they insert the replacement exactly as written, so
`rlw '(a)' '[$1]'` writes `[$1]` where `raw` writes the captured text. This
difference is known and kept for compatibility.

| Written | Inserts |
|---|---|
| `$1`, `${1}` | capture group 1 |
| `$$` | a literal `$` |
| `$5.00` | group 5 (empty when there is none), then `.00` |
| `$1x` | the group **named** `1x` (empty); write `${1}x` |

To insert a price with `raw`, `rfw` or `rcw`, write `raw price '$$5.00';`; with
`rlw` and `rnw` write `$5.00`. The same applies when the replacement comes from
a variable.
