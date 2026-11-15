# Text pipeline syntax: words, quotes and escapes

A `.textforge` file holds one instruction per line, and every instruction ends
with `;`. A line is split into words by `src/text/line_parser.rs` (`split_line`),
which follows the rules of a POSIX shell command line (the rules the
`shell-words` crate implemented up to 0.3.0) plus control-character escapes
inside double quotes.

## Rules

| Written | Unquoted | `'single quotes'` | `"double quotes"` |
|---|---|---|---|
| space or tab | separates words | literal | literal |
| `\n` `\t` `\r` `\0` | **error** | kept as written | newline, tab, carriage return, NUL |
| `\\` `\"` `\$` `` \` `` | the second character | kept as written | the second character |
| `\` + any other character | the second character | kept as written | kept as written (`"\d"` is `\d`) |
| `#` at the start of a word | comment to the end of the line | literal | literal |
| a real line break | **error** | **error** | **error** |

Spaces inside an expression written outside quotes, `[[ ... ]]`, do not
separate words: `onln [[__COUNTER__ + 1]] apply tbs;` has four words, the same
as `onln '[[__COUNTER__ + 1]]' apply tbs;`. This applies only when the closing
`]]` is on the same line; a lone `[[` is ordinary text, as before. Quotes and
escapes inside the expression follow the rules above. Variable templates
(`{{name}}`) take no spaces and are not grouped.

```textforge
atb "banana\nlaranja";     // adds "banana", a newline and "laranja"
raw " " "\t";              // spaces become tabs
raw "\s+" "\n";            // one word per line
atb 'a\nb';                // single quotes: a, backslash, n, b
atb "\\n";                 // backslash and n
klm "^\d+$";               // regex escapes reach the regex unchanged
```

These fail with a `TextParsingError` instead of producing a wrong result:

```textforge
atb banana\nlaranja;       // \n outside double quotes
atb "banana\nlaranja;"     // the line does not end with ';' (it is inside the quotes)
```

## Regex parameters

Patterns are written the same way. Inside double quotes `\n`, `\t` and `\r`
become the characters themselves, which a regex matches exactly as it matches
the escape, so existing patterns keep matching the same text. The one
difference is the `(?x)` flag, which ignores literal whitespace: there, write
`\\n` (or use single quotes) to give the regex its own `\n`.

## Writing pipelines back to text

`quote` writes a parameter so that `split_line` reads it back unchanged. Its
output is the same as `shell_words::quote` except for values with a line break,
which are written in double quotes with escapes (`"a\nb"`). Pipelines with such
parameters can therefore be exported to `.textforge`; before 0.3.1 this was a
`SerializationError`.

Instructions that insert one line (`inl`, `rln`, `apl`, `pln`) still reject
text with line breaks; see [logical lines](lines.md).

## Compatibility with 0.3.0

Lines that use none of the new escapes are split exactly as before. This is
checked by differential tests against `shell_words` over every pipeline in the
repository and 200,000 generated lines (`src/text/line_parser/tests.rs`). What
changed:

- `"\n"`, `"\t"`, `"\r"` and `"\0"` in double quotes are the control
  characters (before: a backslash and a letter).
- `\n`, `\t`, `\r` and `\0` outside quotes are errors (before: the letter alone,
  `\n` became `n`).
- A real line break inside an instruction passed to `read_from_text` is an
  error; reading a file already failed on it.
- After 0.3.1: spaces inside an unquoted `[[ ]]` stay in the word (before:
  they split it, and such lines failed to parse). The differential tests skip
  lines with spaced expressions.

Benchmark: `scripts/benchmark_line_parser.py` (results in
[benchmarks/line-parser](benchmarks/line-parser/README.md)).
