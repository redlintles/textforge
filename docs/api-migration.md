# API migration for the next release

These contract changes were explicitly approved during the builder cleanup.

## Instruction parameter binding

`InstructionMethods::from_params` is renamed to `set_params`. Parameter inputs
accept slices instead of references to vectors; existing owned parameter storage
and `get_params()` return types are unchanged. External instruction implementations
must rename the method and update its signature.

```rust
use textforge::tokens::{InstructionMethods, transforms::atb::Atb};

let mut instruction = Atb::new("🍌");
let parameters = instruction.get_params().clone();
instruction.set_params(parameters.as_slice()).unwrap();
assert_eq!(instruction.transform("x".into(), None).unwrap(), "🍌x");
```

Helpers that only read instruction vectors now take slices. Parameter formatting
uses `Display`, preserving `.to_string()` through Rust's standard `ToString`
implementation. `TextForgeProcessor::default()` is equivalent to `new()`.

## Internal and test-only helpers

The normal build no longer compiles unused context-cleanup placeholders and
registry introspection/owned-token helpers. These are retained under `test_access`,
where the containing modules are already publicly exposed for tests. No production
public module was removed. The global registry still checks name/opcode collisions
while constructing its lookup maps.

## Bytecode output reliability

The bytecode writer now uses `Write::write_all` for headers and records. Tests use
an output sink accepting one byte per write and a failing sink to verify complete
output and error propagation. File framing and opcode mappings are unchanged by
this cleanup.

Useful references:

- [`InstructionMethods`](../src/tokens/mod.rs): universal instruction contract.
- [`TextForgeProcessor`](../src/api/processor.rs): pipeline registration/execution.
- [`Write::write_all`](https://doc.rust-lang.org/std/io/trait.Write.html#method.write_all):
  standard short-write handling used by the writer.
- [Builder design](builder-validation.md): hierarchy, validation, and lowering.
