# Logical line operations

Line numbers start at **1**. Ranges include both endpoints. A missing line,
zero index, or reversed range returns `TextForgeError`. `inl` additionally accepts
line count + 1 (including position 1 in empty input).

An empty string contains zero lines. A trailing terminator belongs to the last
line: `a\n` is one line, `a\n\n` is two. Recognized terminators are LF, CRLF
(atomic), CR, VT, FF, NEL, LINE SEPARATOR and PARAGRAPH SEPARATOR. Other Unicode
content, combining sequences and emoji are unchanged.

Deletion, selection and filtering retain the exact terminators of surviving
lines. Insertion uses the first existing terminator, or LF when none exists.
Append preserves whether a nonempty document ends in a terminator. Empty inserted
text represents a real blank line, including at EOF. Inserted/replacement text
must contain no recognized line breaks; use multiple instructions for more lines.
Replacing an unterminated final line with empty text adds a terminator to represent
that blank line. `joinln` accepts any separator, including line breaks, and places
it only between logical lines. `nln` removes every recognized terminator.

`dle` removes exactly empty lines, retaining whitespace-only lines. Regex filters
use the regex crate's syntax, match line content without its terminator and are
case-sensitive unless flags specify otherwise; use anchors for full-line matches.
`uniqln` compares exact content (without terminators), preserving the first
occurrence; normalization is explicit through `unorm`. `revln` reverses contents
and retains the original sequence of terminators, including the final newline
convention. No instruction sorts lines.

Tokens are available in textual/bytecode parsers, the Rust fluent builder
(including block and conditional bodies), generic portable `InstructionSpec`,
prepared pipelines, watchers and buffered execution (whole-document fallback).
Parameters support the existing variables and interpolation rules.

| DSL | Rust / Python method | Purpose |
| --- | --- | --- |
| `apl "b"` | `append_line` | Append one logical line, preserving the final-terminator convention. |
| `dln 1` | `delete_line` | Delete one line, including its terminator. |
