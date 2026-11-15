#!/usr/bin/env python3
"""Generate the instruction index in the rustdoc (src/tokens/mod.rs, the
module lists of src/tokens/{transforms,instructions}/mod.rs), in README.md and
in the catalog of the `txfg` game (src/bin/txfg/catalog.rs).

CATALOG is the single list of instructions with their syntax and a one-line
description. Add a new instruction here and run
`python3 scripts/instruction_index.py`; tests/instruction_index.rs fails when
an instruction registered in src/globals/table.rs is missing.
"""
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]

CATALOG = [
 ("Add, insert and repeat", [
  ("atb", "atb {text};", "Add `text` to the beginning."),
  ("ate", "ate {text};", "Add `text` to the end."),
  ("ins", "ins {index} {text};", "Insert `text` after the character at `index`."),
  ("padl", "padl {text} {length};", "Pad on the left with `text` up to `length` characters."),
  ("padr", "padr {text} {length};", "Pad on the right with `text` up to `length` characters."),
  ("rpt", "rpt {times};", "Repeat the text `times` times."),
  ("rev", "rev;", "Reverse the characters."),
  ("rtl", "rtl {times};", "Rotate the characters `times` positions to the left."),
  ("rtr", "rtr {times};", "Rotate the characters `times` positions to the right."),
  ("splc", "splc;", "Separate every character with a space."),
  ("rmws", "rmws;", "Remove all whitespace."),
 ]),
 ("Delete", [
  ("dlf", "dlf;", "Delete the first character."),
  ("dll", "dll;", "Delete the last character."),
  ("dls", "dls {index};", "Delete the character at `index`."),
  ("dlc", "dlc {start} {end};", "Delete the characters `start..=end`."),
  ("dla", "dla {index};", "Delete every character after `index`."),
  ("dlb", "dlb {index};", "Delete every character before `index`."),
 ]),
 ("Replace and match (regular expressions)", [
  ("raw", "raw {pattern} {text};", "Replace every match of `pattern` with `text`."),
  ("rfw", "rfw {pattern} {text};", "Replace the first match."),
  ("rlw", "rlw {pattern} {text};", "Replace the last match."),
  ("rnw", "rnw {pattern} {text} {n};", "Replace the `n`th match."),
  ("rcw", "rcw {pattern} {text} {count};", "Replace the first `count` matches."),
  ("rmp", "rmp {pattern};", "Remove every match of `pattern`."),
  ("emj", "emj {pattern} {separator};", "Keep only the matches, joined with `separator`."),
  ("rmape", "rmape {map};", "Apply a replacement map defined with `rmapd`, without cascading."),
 ]),
 ("Trim and select", [
  ("tbs", "tbs;", "Trim whitespace on both sides."),
  ("tls", "tls;", "Trim whitespace on the left."),
  ("trs", "trs;", "Trim whitespace on the right."),
  ("slt", "slt {start} {end};", "Keep only the characters `start..=end`."),
  ("sslt", "sslt {pattern} {index};", "Split by `pattern` and keep piece `index`."),
 ]),
 ("Case", [
  ("tua", "tua;", "Uppercase everything."),
  ("tla", "tla;", "Lowercase everything."),
  ("tucs", "tucs {index};", "Uppercase the character at `index`."),
  ("tlcs", "tlcs {index};", "Lowercase the character at `index`."),
  ("tucc", "tucc {start} {end};", "Uppercase the characters `start..=end`."),
  ("tlcc", "tlcc {start} {end};", "Lowercase the characters `start..=end`."),
  ("tucw", "tucw {index};", "Uppercase word `index`."),
  ("tlcw", "tlcw {index};", "Lowercase word `index`."),
  ("cfw", "cfw;", "Capitalize the first word."),
  ("clw", "clw;", "Capitalize the last word."),
  ("cts", "cts {index};", "Capitalize word `index`."),
  ("ctc", "ctc {start} {end};", "Capitalize the words in the characters `start..=end`."),
  ("ctr", "ctr {start} {end};", "Capitalize words `start..=end`."),
  ("jkbc", "jkbc;", "Join the words as kebab-case."),
  ("jsnc", "jsnc;", "Join the words as snake_case."),
  ("jcmc", "jcmc;", "Join the words as camelCase."),
  ("jpsc", "jpsc;", "Join the words as PascalCase."),
 ]),
 ("Encoding and Unicode", [
  ("urle", "urle;", "URL-encode."),
  ("urld", "urld;", "URL-decode."),
  ("htmle", "htmle;", "Escape HTML special characters."),
  ("htmlu", "htmlu;", "Unescape HTML entities."),
  ("jsone", "jsone;", "Escape as a JSON string."),
  ("jsonu", "jsonu;", "Unescape a JSON string."),
  ("unorm", "unorm {mode};", "Normalize Unicode to `nfc`, `nfd`, `nfkc` or `nfkd`."),
 ]),
 ("Lines (line numbers start at 1)", [
  ("apl", "apl {text};", "Append a line."),
  ("pln", "pln {text};", "Prepend a line."),
  ("inl", "inl {line} {text};", "Insert a line before `line` (one past the last appends)."),
  ("rln", "rln {line} {text};", "Replace the content of `line`, keeping its terminator."),
  ("dln", "dln {line};", "Delete `line`."),
  ("dlna", "dlna {line};", "Delete the lines after `line`."),
  ("dlnb", "dlnb {line};", "Delete the lines before `line`."),
  ("dlni", "dlni {start} {end};", "Delete lines `start..=end`."),
  ("slnc", "slnc {start} {end};", "Keep only lines `start..=end`."),
  ("dle", "dle;", "Delete empty lines."),
  ("klm", "klm {pattern};", "Keep the lines that match `pattern`."),
  ("dlm", "dlm {pattern};", "Delete the lines that match `pattern`."),
  ("revln", "revln;", "Reverse the order of the lines."),
  ("uniqln", "uniqln;", "Keep the first occurrence of each line."),
  ("joinln", "joinln {separator};", "Join the lines with `separator`."),
  ("nln", "nln;", "Remove every line terminator."),
 ]),
 ("Files", [
  ("wtf", "wtf {file};", "Write the text to `file` and keep it unchanged."),
  ("apf", "apf {file};", "Append the contents of `file`."),
  ("ppf", "ppf {file};", "Prepend the contents of `file`."),
  ("inf", "inf {file} {index};", "Insert the contents of `file` after the character at `index`."),
  ("ppjn", "ppjn {pipeline};", "Run the pipeline in another file on the text."),
 ]),
 ("Variables", [
  ("var", "var {name} = {value};", "Define a mutable variable."),
  ("val", "val {name} = {value};", "Define an immutable variable."),
  ("mutv", "mutv {name} = {value};", "Change a mutable variable."),
  ("eval", "eval {expression} in {name};", "Evaluate an expression into a variable."),
 ]),
 ("Control flow", [
  ("blk", "blk {name} assoc {instruction};", "Add an instruction to block `name`."),
  ("cblk", "cblk {name};", "Run the instructions of block `name`."),
  ("ifdc", "ifdc {text} do {instruction};", "Run `instruction` if the text contains `text`."),
  ("iter", "iter {times} times {instruction};", "Run `instruction` `times` times (`__COUNTER__` counts from 0)."),
  ("onln", "onln {line} apply {instruction};", "Run `instruction` on one line only."),
  ("onck", "onck {start} {end} apply {instruction};", "Run `instruction` on the characters `start..=end` only."),
  ("onlni", "onlni {start} {end} apply {instruction};", "Run `instruction` on lines `start..=end` as one section."),
  ("rmapd", "rmapd {map} add {pattern} {text};", "Add a rule to replacement map `map`."),
  ("end", "end;", "Stop the pipeline (or the current block or loop)."),
  ("null", "null;", "Do nothing."),
 ]),
]


def kind(mnemonic):
    return 'transforms' if (ROOT / 'src/tokens/transforms' / mnemonic).is_dir() else 'instructions'


def struct(mnemonic):
    source = (ROOT / 'src/tokens' / kind(mnemonic) / mnemonic / 'mod.rs').read_text()
    return re.search(r'pub struct (\w+)', source).group(1)


def replace_between(text, begin, end, body):
    start = text.index(begin) + len(begin)
    return text[:start] + body + text[text.index(end, start):]


def rustdoc_index():
    lines = ['\n//!']
    for title, rows in CATALOG:
        lines += [f'//! ### {title}', '//!', '//! | Instruction | Syntax | Effect |', '//! |---|---|---|']
        for mnemonic, syntax, description in rows:
            link = f'{kind(mnemonic)}::{mnemonic}::{struct(mnemonic)}'
            lines.append(f'//! | [`{mnemonic}`]({link}) | `{syntax}` | {description} |')
        lines.append('//!')
    return '\n'.join(lines) + '\n'


def readme_index():
    lines = ['\n']
    for title, rows in CATALOG:
        lines += [f'### {title}', '', '| Instruction | Syntax | Effect |', '|---|---|---|']
        lines += [f'| `{m}` | `{s}` | {d} |' for m, s, d in rows]
        lines.append('')
    return '\n'.join(lines)


def module_list(kind_name):
    path = ROOT / 'src/tokens' / kind_name / 'mod.rs'
    descriptions = {m: (s, d) for _, rows in CATALOG for m, s, d in rows}
    out = []
    for line in path.read_text().splitlines():
        if line.startswith('/// `') and ' — ' in line:
            continue
        match = re.fullmatch(r'pub mod (\w+);', line)
        if match and match.group(1) in descriptions:
            syntax, description = descriptions[match.group(1)]
            out.append(f'/// `{syntax}` — {description}')
        out.append(line)
    path.write_text('\n'.join(out) + '\n')


HEADER = re.compile(r'^/// ?(Token `\w+` —|`[a-z]+[ ;][^`]*` —|[A-Z][A-Za-z]{1,6} ?[-—] )')


def struct_header(mnemonic, syntax, description):
    """Give the instruction's struct a first doc line: its syntax and effect."""
    path = ROOT / 'src/tokens' / kind(mnemonic) / mnemonic / 'mod.rs'
    lines = path.read_text().split('\n')
    name = struct(mnemonic)
    end = next(i for i, l in enumerate(lines) if l.startswith(f'pub struct {name}'))
    start = end
    while start > 0 and (lines[start - 1].startswith(('///', '#[')) or (lines[start - 1] == '' and start > 1 and lines[start - 2].startswith('///'))):
        start -= 1
    header = f'/// `{syntax}` — {description}'
    first = next((i for i in range(start, end) if lines[i].startswith('///')), None)
    if first is not None and HEADER.match(lines[first]):
        lines[first] = header
    else:
        at = first if first is not None else start
        lines[at:at] = [header] + (['///'] if first is not None else [])
    path.write_text('\n'.join(lines))


def rust_str(text):
    return '"' + text.replace('\\', '\\\\').replace('"', '\\"') + '"'


def game_catalog():
    """The catalog `txfg` shows with `!what` (src/bin/txfg/catalog.rs)."""
    lines = [
        '//! Every instruction with its syntax and effect, for `!what`.',
        '//!',
        '//! Generated by `scripts/instruction_index.py` from its `CATALOG`; do not edit.',
        '',
        '/// One instruction: mnemonic, syntax, a one-line effect and its group.',
        '#[derive(Debug, Clone, Copy, PartialEq, Eq)]',
        'pub struct Entry {',
        '    /// The mnemonic, e.g. `tbs`.',
        '    pub name: &\'static str,',
        '    /// The syntax, e.g. `tbs;`.',
        '    pub syntax: &\'static str,',
        '    /// What the instruction does.',
        '    pub effect: &\'static str,',
        '    /// The group of the instruction index.',
        '    pub group: &\'static str,',
        '}',
        '',
        '/// Every instruction, in the order of the instruction index.',
        '#[rustfmt::skip]',
        'pub const CATALOG: &[Entry] = &[',
    ]
    for title, rows in CATALOG:
        for mnemonic, syntax, description in rows:
            lines.append(f'    Entry {{ name: {rust_str(mnemonic)}, syntax: {rust_str(syntax)}, '
                         f'effect: {rust_str(description)}, group: {rust_str(title)} }},')
    lines += ['];', '']
    (ROOT / 'src/bin/txfg/catalog.rs').write_text('\n'.join(lines))


def main():
    tokens = ROOT / 'src/tokens/mod.rs'
    tokens.write_text(replace_between(tokens.read_text(), '//! <!-- instruction-index:begin -->',
                                      '//! <!-- instruction-index:end -->', rustdoc_index()))
    readme = ROOT / 'README.md'
    readme.write_text(replace_between(readme.read_text(), '<!-- instruction-index:begin -->',
                                      '<!-- instruction-index:end -->', readme_index()))
    for kind_name in ('transforms', 'instructions'):
        module_list(kind_name)
    game_catalog()
    for _, rows in CATALOG:
        for mnemonic, syntax, description in rows:
            struct_header(mnemonic, syntax, description)


if __name__ == '__main__':
    main()
