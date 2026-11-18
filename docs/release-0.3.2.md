# TextForge 0.3.2 core release

The core package version is **0.3.2**, published to crates.io on 2026-11-15
(tag `v0.3.2`). It fixes the problems found by using the published 0.3.1
`txf` on everyday tasks. Several fixes change behavior that existing
pipelines can notice, so read the **Behavior changes** of the
[changelog](../changelog.md) before upgrading.

## Scope

- **Every index starts at 0.** Line numbers of `dln`, `dlna`, `dlnb`, `dlni`,
  `slnc`, `inl`, `rln`, `onln` and `onlni` (and their builder methods) now
  count from 0, like characters and words. Pipelines, portable plans and
  bytecode that use them must subtract one from each line number.
- **Interpolation keeps stray delimiters**: `{{`, `}}`, `[[` and `]]` that form
  no variable read or expression stay in the text, so JSON and POSIX regex
  classes work unescaped. See [text syntax](text-syntax.md).
- **Ranges and indices follow one set of rules**
  ([indices and ranges](indices-and-ranges.md)): line ranges clamp an end past
  the last line, `ins`/`inf` with index `0` insert at the start, `inf` takes
  the index first, and `rlw`/`rnw` expand `$1` like the other replacements.
- **Paths are relative to the pipeline file** for `wtf`, `apf`, `ppf` and
  `inf`, like `ppjn`.
- `end` inside `cblk` or `iter` stops only that block or loop; `val` cannot be
  defined twice; expression results must be integers or text; `len(text)`
  counts characters.
- **Language additions**: comments after an instruction and whole-line `#`
  comments, `txf --chomp`, and the `txfg` game (`game` feature).
- No binding repository was modified.

## Validation

The release revision passed the full pre-commit checklist of `AGENTS.md`
(section 14) with every feature set.
