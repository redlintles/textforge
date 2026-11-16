# TextForge documentation

The API reference, with every type, method and instruction, is on
[docs.rs](https://docs.rs/textforge). This folder holds the guides, the design
notes behind them and the evidence (benchmarks and audits) for past changes.
Everything here is in English.

## Guides

How the language and the library behave.

| Guide | Covers |
|---|---|
| [Text syntax](text-syntax.md) | Words, quotes and escapes (`"\n"`, `"\t"`) in `.textforge` lines |
| [Logical lines](lines.md) | Line terminators, line numbers and the line instructions |
| [Indices, ranges and replacement text](indices-and-ranges.md) | Bases, what happens out of range, argument order, regex patterns and `$` in replacements |
| [Section instructions](on-instructions.md) | `onln`, `onck`, `onlni`: applying one instruction to part of the text |
| [File instructions](file-instructions.md) | `wtf`, `apf`, `ppf`, `inf` and pipeline joins (`ppjn`) |
| [Replacement maps](replace-maps.md) | `rmapd`/`rmape`: several non-cascading replacements in one pass |
| [Unicode normalization](unicode-normalization.md) | `unorm` and the four normalization forms |
| [Execution context and built-in variables](execution-context.md) | Variables, `__LINES__`, `__PIPELINE_NAME__` and the other built-ins |
| [Execution profiles and `.txfrc`](execution-profiles.md) | Memory use, spooling and safety limits; the `.txfrc` file |
| [Debug output](debug-output.md) | The step-by-step trace of `process_all_with_debug` |
| [Portable API](portable-api.md) | The owned, language-neutral API the bindings use |
| [Portable API parity](portable-parity.md) | How each Rust method maps to the portable API |
| [The `txfg` game](txfg.md) | Playing, game files, scoring and the difficulty rating |
| [Bindings: Python](python-bindings.md) · [Node.js](node-bindings.md) | Where the bindings live and how they are versioned |

## Releases

- [Changelog](../changelog.md), with upgrade notes for each release.
- [0.3.1 release notes](release-0.3.1.md) · [0.3.0 release notes](release-0.3.0.md) · [0.2.0 release notes](release-0.2.0.md)
- [API migration in 0.2.0](api-migration.md)

## Design notes

Why the engine works the way it does. Useful when changing it.

| Note | Covers |
|---|---|
| [Tree-based pipeline construction](builder-validation.md) | How builders validate and lower instruction trees |
| [Extending structural validation](validation-rules.md) | The nesting and placement rules, and how to add one |
| [Prepared constants and shared statistics](prepared-pipelines-and-shared-statistics.md) | Binding instructions once per pipeline; compiled templates |
| [Allocation optimization](allocation-optimization.md) | Borrowed results and buffer reuse (0.2.0) |
| [Buffered adapters](buffered-adapters.md) · [TextBuffer execution](tempfile-experiment.md) | The `buffered` feature and its spooled documents |
| [Safe file I/O](safe-file-io.md) | Reading and writing files without `mmap`, bounded batches |
| [Watcher report reliability](watcher-report-reliability.md) | How watcher reports are collected and written |
| [Panic fixes](panic-fixes.md) | Crash paths fixed before 0.2.0 |
| [`.txfrc` schema](txfrc.schema.json) | JSON schema of the configuration file |

## Benchmarks

Reproducible measurements behind performance changes. Each report names its
script in [`scripts/`](../scripts) and its raw results.

| Report | Measures |
|---|---|
| [Engine (0.3.0)](benchmarks/engine/README.md) | Parameter binding, templates, expressions and loading, before and after the 0.3.0 engine work |
| [Section instructions](benchmarks/on-instructions/README.md) | `onln`, `onck`, `onlni` |
| [Built-in variables](benchmarks/builtin-variables/README.md) | Cost of the static context |
| [Line parser (0.3.1)](benchmarks/line-parser/README.md) | `split_line` against `shell_words::split` |
| [Line performance](line-performance-summary.md) ([full report](benchmarks/line-performance/report.md)) | The line instructions in 0.2.0 |
| [Batch input](benchmarks/batch-io/report.md) · [Safe file I/O](benchmarks/safe-file-io/report.md) | File reading strategies |
| [Watcher reliability](benchmarks/watcher-reliability/report.md) · [Watchers comparison](benchmarks/watchers-comparison-report.md) | Watcher overhead |
| [Revision comparison](benchmarks/comparison-report.md) · [Buffered comparison](benchmarks/buffered-comparison-report.md) · [Steps 3–5](benchmarks/steps-345/report.md) · [Follow-up notes](benchmarks/internal-improvements.md) | Earlier revision-to-revision comparisons |

## Audits

Internal reviews: each finding with its reproduction, the fix and the commit.

- [0.3.0: `src/tokens`](audits/0.3.0-tokens-audit.md)
- [0.3.0: section instructions, debug trace and interpolation](audits/0.3.0-on-debug-audit.md)
- [0.3.2: the `txf` command-line program](audits/0.3.2-cli-audit.md)
- [Python binding rules](python-binding-audit.md)
