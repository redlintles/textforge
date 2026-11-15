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
| `#` at the start of a word | comment to the end of the line (quote it to use it: `raw "#" x;`) | literal | literal |
| a real line break | **error** | **error** | **error** |

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

## Comments

```textforge
// a whole-line comment
# also a whole-line comment
atb "> ";      // a comment after the instruction
ate "!";       # this form works too
```

A comment after an instruction starts after the `;` that ends it. Inside
quotes `//` and `#` are text (`atb "// not a comment";`). Up to 0.3.1 only
whole-line `//` comments were accepted, and a comment after the `;` failed
with "an instruction must end with `;`".

A pipeline file may start with a UTF-8 byte order mark (some editors on
Windows add one); it is ignored.

## `{{ }}` and `[[ ]]` in parameters

`{{name}}` reads a variable and `[[expression]]` evaluates an expression, in
any kind of quotes. Text that only looks like one stays as written:

| Written | Result |
|---|---|
| `{{name}}` (a letter or `_`, then letters, digits, `_`) | the variable, or `VariableNotFound` |
| `{{user.name}}`, `{{ name }}`, `{{1a}}`, `{{}}` | unchanged: not a variable name |
| `{"a":{"b":1}}`, `m[i[0]]`, a lone `{{` or `]]` | unchanged |
| `[[1 + 1]]` | `2` |
| `[[:alpha:]]`, `[[:alpha:][:digit:]]`, `[[]]` | unchanged: no expression is empty or starts with `:` |
| `\{{name}}`, `\[[1 + 1]]` | `{{name}}`, `[[1 + 1]]`: the backslash escapes the opening |

Up to 0.3.1, delimiters that formed no marker were deleted, which truncated
JSON and broke regex character classes.

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

Benchmark: `scripts/benchmark_line_parser.py` (results in
[benchmarks/line-parser](benchmarks/line-parser/README.md)).
