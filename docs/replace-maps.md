# Replacement maps

Define a reusable named set of regular-expression replacements in the execution
context, then apply it in one non-cascading operation:

```txf
rmapd animals add 'cat' 'wolf';
rmapd animals add 'wolf' 'dog';
rmape animals;
```

Input `cat wolf` becomes `wolf dog`. Inserted replacements are never matched again
within the same `rmape`. Calling `rmape` again operates on the current text.

`rmapd NAME add REGEX REPLACEMENT;` creates a map on its first executed definition.
An identical regex string updates its replacement without changing its priority;
different regex strings append rules in definition order. Definitions execute
conditionally inside `ifdc`. They are allowed only globally or inside conditionals, and forbidden below `iter`
or anywhere below `blk`, including
inside a conditional in a block. `rmape NAME;` is permitted inside blocks,
conditionals and iterations. An undefined map produces `ValidationError` at
`rmape`; an invalid regex produces `TextParsingError` at `rmapd`.

Matches are chosen by earliest byte position in the original input. Ties use
rule definition order, regardless of match length. A consumed match suppresses
other overlapping matches. Searches preserve the original string for anchors and
word boundaries. Empty matches are supported at Unicode boundaries; an empty
match immediately following an accepted match at its end is suppressed, following
`regex::Regex::replace_all`'s adjacent-empty-match convention. A nonempty match
may follow an empty match at the same position. Captures use the regex crate's
replacement syntax: `$1`, `${name}` and `$$` for a literal dollar sign. There is
no normalization of Unicode. Parameters use normal TextForge variable and math
interpolation and shell quoting.

## Rust and portable APIs

```rust
use textforge::api::{TextForgeBuilderMethods,
    processor::{TextForgeProcessor, TextForgeProcessorMethods}};
let mut processor = TextForgeProcessor::new();
let id = processor.create_pipeline()
    .define_replace_map("animals", "cat", "wolf").unwrap()
    .define_replace_map("animals", "wolf", "dog").unwrap()
    .execute_replace_map("animals").unwrap().build().unwrap();
assert_eq!(processor.process_all(&id, "cat wolf").unwrap(), "wolf dog");
```

Portable clients use `InstructionSpec::new("rmapd", [name, regex, replacement])`
and `InstructionSpec::new("rmape", [name])`, with owned strings and argument
vectors. The syntax literal `add` is omitted. Both instructions support text and
big-endian bytecode persistence (opcodes `0x3f` and `0x40`). Existing opcodes and
protocol layout are unchanged. The Python methods are `define_replace_map` and
`execute_replace_map` with the same parameters; `Pipeline.add` also accepts the
mnemonics directly. Errors retain portable codes and stages.

## Reuse, concurrency and cost

Map membership and replacements belong to each execution context. They never leak
between inputs, pipelines or batch tasks. Compiled regexes are immutable and shared
through `Arc`; constant definitions use the processor's prepared instruction path.
A synchronized cache shared by instruction-template clones also reuses resolved
patterns in nested or interpolated definitions and simultaneous batch tasks.
Compilation is serialized on a cache miss, so simultaneous misses do not compile
the same cached pattern twice. The cache holds at most 64 patterns and clears when
full; evicted dynamic patterns can be recompiled. Prepared instructions and active
maps retain their own regex references. No additional dependency is required.

Execution retains at most one capture candidate per rule and one output string,
rather than materializing every match. Candidate selection scans the rules; after
a match, invalidated candidates are searched from the new cursor. This preserves
overlap correctness but is not a single regex automaton: large rule sets and
expensive patterns can require repeated searches. There is no claim of linear
runtime in both rule count and input size. Buffered execution uses the existing
whole-document fallback. File batches use the normal Rayon processor path and
independent contexts; they share regex compilation, not mutable map contents.

Run the reproducible cache benchmark with:

```sh
cargo test --release --test replace_map_benchmark -- --ignored --nocapture
```

It reports warmed registered execution versus reconstructing/compiling the same
pipeline for each document. Timings are informational and have no flaky threshold.
