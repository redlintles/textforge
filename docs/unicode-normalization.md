# Unicode normalization (`unorm`)

`unorm MODE;` normalizes the current complete text. Supported modes are exactly
`nfc`, `nfd`, `nfkc` and `nfkd`, in lowercase; the parameter is required in the DSL.
It accepts standard TextForge interpolation and variable references.

| Mode | Operation | Example |
| --- | --- | --- |
| `nfc` | Canonical decomposition followed by canonical composition | `e` + U+0301 becomes `é` |
| `nfd` | Canonical decomposition and canonical ordering | `é` becomes `e` + U+0301 |
| `nfkc` | Compatibility decomposition followed by canonical composition | `Ａﬁ²é` becomes `Afi2é` |
| `nfkd` | Compatibility decomposition and canonical ordering | `Ａﬁ²é` becomes `Afi2e` + U+0301 |

NFC/NFD retain compatibility distinctions. NFKC/NFKD can discard meaningful
formatting distinctions, such as fullwidth forms and superscripts. Normalization
preserves canonical accents; it does not perform case folding, transliteration,
diacritic removal or general confusable-character detection. It is locale-independent
and idempotent. It does not insert stream-safe Combining Grapheme Joiners.

Normalization can change byte length and scalar count. It does not turn existing
character-indexed operations into grapheme-cluster-indexed operations. A later
instruction can produce non-normalized text again. TextForge does not implicitly
normalize input, regex patterns, replacements, variable values or other parameters.
Place `unorm` where the pipeline needs it, and use matching representations in
patterns and values:

```txf
unorm nfc;
rmapd accents add 'é' 'E';
rmape accents;
```

Both composed `é` and decomposed `e` + U+0301 then match the rule. The normalization
instruction is allowed globally and inside blocks, conditionals and iterations.
It needs no execution context and has no mutable shared execution state. Empty
text, emoji, variation selectors, ZWJ sequences and embedded NUL are supported.
Normalization follows Unicode rules for every script, including Hangul composition
and decomposition and canonical ordering of leading combining marks.

## Public Rust API

```rust
use textforge::api::{
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
    TextForgeBuilderMethods,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut processor = TextForgeProcessor::new();
    let id = processor.create_pipeline()
        .normalize_unicode("nfc")?
        .build()?;
    assert_eq!(processor.process_all(&id, "Cafe\u{301}")?, "Café");
    Ok(())
}
```

The lower-level `Unorm::default()` uses NFC; `Unorm::new(mode)` returns a checked
instruction. Binding unresolved interpolation retains it for construction/export;
directly executing that unresolved instruction returns an error. Processor-based
execution resolves mode parameters before applying the instruction.

## Portable API and Python

The portable representation is
`InstructionSpec::new("unorm".into(), vec![Argument::Text("nfc".into())])`.
`Argument::Variable("form".into())` defers the mode to execution. The generic
portable instruction contract is unchanged; validation and normalization stay in
the core. Invalid modes produce `InvalidParameters` with instruction `unorm` and
the offending mode as diagnostic input. Through portable APIs this is an execution
error; wrong arity or literal types can be rejected earlier during validation.

```python
from textforge import Pipeline

normalize = Pipeline().normalize_unicode("nfc").compile()
assert normalize("Cafe\u0301") == "Café"
```

The Python facade is explicitly typed and also accepts `Variable`. It forwards
portable instructions rather than using Python's own Unicode tables. Text and
bytecode persistence retain the mode (including variable references); opcode
`0x41` uses the existing big-endian protocol without changing its layout.

## Dependency, performance and document boundaries

The approved `unicode-normalization` dependency requirement is `0.1.26`; the
checked-in lockfile currently selects 0.1.26, with Unicode **18.0.0** tables. Its
transitive storage dependency is `tinyvec`. The conformance tests check the table
version explicitly, so a table update requires reviewing/updating the corpus.

A Unicode QuickCheck returns the original `Cow` immediately when it proves that
no change is needed. For other results, a normalization iterator is compared with
the source until the first difference; only then is an output string allocated,
reusing the matching prefix and consuming the remaining iterator once. If the
iterator produces identical text, the original ownership/allocation is retained.
This avoids repeating normalization after an uncertain QuickCheck result, but
still adds a scan and is not universally faster than unconditional normalization.
No per-document regex compilation or normalization cache is needed. Constant modes
use the existing prepared-instruction path; concurrent calls and file-batch tasks
operate independently.

Buffered execution uses the existing whole-document fallback. It materializes the
complete input before normalization, preserving combining sequences across copy
boundaries. It does not normalize individual chunks independently and does not
provide a bounded-memory streaming normalizer. Long combining sequences can also
require internal normalization storage. A separate streaming implementation would
need to carry normalization state across chunks.

## Validation and benchmark

- Adjacent unit tests cover all modes, Hangul, combining order, emoji, empty input,
  invalid parameters, idempotence, retained allocations and big-endian serialization.
- Integration tests cover public/portable parity, variables, nesting, concurrent
  reuse, text/bytecode round trips, file batches and cross-boundary spilled buffers.
- The vendored official Unicode 18 corpus is checked offline against all four
  forms and its invariants; unlisted Unicode scalars must remain unchanged.
  See [fixture provenance](../tests/data/unicode/README.md).
- Python tests compare against independent public-Rust reference results and
  verify the installed wheel and the independently rebuilt source distribution.

Run the reproducible informational benchmark with:

```sh
cargo test --release --test unicode_normalization_benchmark -- --ignored --nocapture
```

It compares NFC normalizedness checking/borrowing with unconditional collection
on ASCII, normalized accented text and decomposed text. Timings have no pass/fail
threshold and depend on workload and machine.

References: [Unicode UAX #15](https://www.unicode.org/reports/tr15/),
[`unicode-normalization`](https://docs.rs/unicode-normalization/0.1.26/unicode_normalization/).
