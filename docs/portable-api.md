# Portable API for language adapters

`textforge::api::portable` is an additive Rust API between language-specific
bindings and the existing engine. It does not replace the fluent Rust API, expose
parser implementation types, require `test_access`, or define a C ABI.

## Ownership and lifecycle

| Type | Role | Ownership |
| --- | --- | --- |
| `Argument` | Typed text, number, variable reference, or nested instruction | Owned strings and nested descriptions |
| `InstructionSpec` | Canonical mnemonic and ordered effective arguments | Owned description, independent of any processor |
| `PipelinePlan` | Instructions and hierarchical groups under construction | Owns the existing internal tree |
| `CompiledPipeline` | Validated executable | Owns a processor and its private registered pipeline |
| `BindingError` | Code, stage, message, instruction, and diagnostic context | Owned fields, independent of engine lifetimes |
| `ErrorStage` | Conversion, validation, or execution boundary | Small value enum |

All new public types have rustdoc examples beside their definitions in
[`src/api/portable`](../src/api/portable/mod.rs).

1. Construct descriptions and append them to a plan.
2. Move complete subplans into block or conditional groups.
3. Compile the complete plan using the existing structural policies.
4. Reuse the independent compiled object for multiple inputs.
5. Release plans/executables through ordinary Rust ownership in the binding.

Compilation borrows a plan for the duration of the call and can be repeated.
Dropping a plan does not invalidate executables already compiled from it. There
is no persistent `&mut TextForgeProcessor` borrow, self-referential wrapper, global
processor registry, leaked block name, or callback retained by this API.

## Complete example

```rust
use textforge::api::portable::{
    Argument, BindingError, InstructionSpec, PipelinePlan,
};

fn example() -> Result<(), BindingError> {
    let mut plan = PipelinePlan::new();
    plan.add(InstructionSpec::new(
        "val".into(),
        vec![Argument::Text("prefix".into()), Argument::Text("🍌".into())],
    ))?;

    let mut body = PipelinePlan::new();
    body.add(InstructionSpec::new(
        "atb".into(),
        vec![Argument::Variable("prefix".into())],
    ))?;

    let name = format!("cleanup-{}", 42);
    plan.block(name.clone(), body);
    plan.call_block(name)?;
    let pipeline = plan.compile()?;
    drop(plan);

    assert_eq!(pipeline.process("é")?, "🍌é");
    assert_eq!(pipeline.process("")?, "🍌");
    Ok(())
}
```

A block/conditional method consumes its body plan. A foreign wrapper must either
represent that transfer explicitly or mark the transferred object as consumed;
it must not leave an alias pretending to own a moved Rust value. Compiled objects
remain reusable. This is ordinary owned-object handling, without the original
builder's long-lived processor borrow.

## Canonical instructions and typed arguments

`InstructionSpec::name` uses the existing registry's DSL mnemonic, such as `atb`,
`tbs`, `rpt`, `ifdc`, or `iter`. Arguments exclude syntax-only words (`assoc`, `do`,
`times`). The portable layer performs registry lookup rather than maintaining a
second instruction enum or per-transform implementation.

`Argument::Text` is a typed string, with the engine's normal string interpolation.
It does not automatically become a numeric reference because it contains braces;
use `Argument::Variable("n".into())` for an explicit variable parameter, including
when a numeric instruction parameter must be resolved later.

`Argument::Number` is `u64`, converted with a checked conversion to `usize`.
Bindings must reject negative/oversized source-language integers before producing
a `u64`. The layer additionally rejects values that exceed a smaller target's
native index width, rather than truncating them.

Parameter shapes follow the registry. For example, DSL `val`/`var` values use
strings, whereas the fluent Rust helpers accept numbers and normalize them to
strings. A numeric variable declaration can use `Text("3")`; `Number(3)` is not
silently converted into a string by the generic portable API. Language-specific
convenience methods can normalize input or build these concrete descriptions.

`Argument::Instruction(Box<InstructionSpec>)` expresses one nested instruction,
for example the body parameter of `iter`, without crossing a foreign callback:

```rust
use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
let mut plan = PipelinePlan::new();
plan.add(InstructionSpec::new("iter".into(), vec![
    Argument::Number(2),
    Argument::Instruction(Box::new(InstructionSpec::new("atb".into(), vec![
        Argument::Text("🍌".into()),
    ]))),
])).unwrap();
assert_eq!(plan.compile().unwrap().process("x").unwrap(), "🍌🍌x");
```

## Groups and validation

`block(name, body)` stores a dynamically owned name directly in the common tree.
`contains_each(pattern, body)` wraps each child separately; the condition is
re-evaluated after each preceding child. It preserves the fluent API's behavior,
including conditionals inside block bodies.

The portable layer can represent hierarchies that the fluent traits would reject
at compile time. Those hierarchies are rejected by the same registered rules when
`compile()` is called. A raw `blk` instruction cannot bypass ancestor checks.
No processor is created/registered until the entire validation/lowering succeeds.

The shared [validation rules](validation-rules.md) check effective arity, literal
types, block ancestors, and depth. Parser depth limits are unchanged. Nested
instruction-description conversion is itself bounded by the greatest parser depth;
the full tree check subsequently applies the actual ancestor-dependent limit.

Conversion, validation, lowering, and cleanup of nested plans/descriptions use
explicit stacks. Regression coverage includes rejection and cleanup of hierarchies
with 10,000 levels. No recursive conversion or recursive default tree destruction
is required to report these invalid inputs.

Compilation is structural, not proof that every runtime operation will succeed.
References, expressions, regex semantics, missing blocks, and input-dependent
conditions are still evaluated by the existing engine at execution time.

## Errors

| Stage | Examples | When returned |
| --- | --- | --- |
| `Conversion` | Unknown mnemonic, out-of-range native number, excessive description depth | `add`/`call_block` |
| `Validation` | Wrong arity/type, nested blocks, combined group/instruction depth | `compile` |
| `Execution` | Undefined variable/block, invalid regex, input-dependent failure | `process` |

A failed append leaves the existing plan unchanged. A failed compile preserves
the plan and returns no executable. Executing an object does not mutate its plan
or persist an execution context/error history between inputs.

`BindingError` preserves the engine's numeric codes and raw diagnostic data.
Conversion lookup errors identify the requested mnemonic. No terminal color
formatting is added; user-provided text is not sanitized or rewritten. Bindings
should map the fields to exception/result objects instead of parsing `Display`.
`ErrorStage` and `Argument` are non-exhaustive enums so adapters should account for
future variants when matching Rust values.

## Execution and binding boundary

`CompiledPipeline::process(&self, input: &str)` delegates to
`TextForgeProcessorMethods::run_transform` and returns owned text or an owned
error. Each call creates its own execution context and local error manager. The
memory API does not route through the experimental buffered file adapters.

The compiled type is `Send + Sync`; tests share it through `Arc` and execute it
from multiple Rust threads. This does not configure any foreign runtime's locks,
thread attachment, or callback rules. The language-specific binding handles those
rules and keeps input storage alive until the call returns.

Adapt once during construction/compilation, then cross the binding once per
whole pipeline execution. There is no serialization into DSL text and no extra
portable-to-core conversion per instruction during processing. Actual input/output
copying across a foreign runtime depends on the adapter's ownership strategy.

Initial scope covers in-memory construction, compilation, execution, and errors.
File APIs, persistence, watchers, foreign callbacks, cancellation, and a C ABI are
not part of this initial contract. PyO3 or future ABI-specific adapters can depend
on this API without changing current fluent callers. Future public contracts
still require explicit approval under [AGENTS.md](../AGENTS.md).

## Verification and references

The [integration tests](../tests/portable.rs) import only production public paths;
they run without `test_access`. They check equivalence against the fluent API,
Unicode/combining characters/NUL/empty text, owned dynamic names, repeated builds,
references/interpolation, group order, nested instructions, errors, depth,
conversion atomicity, cleanup, and read-only concurrent Rust execution.

- [Portable API and lifecycle](../src/api/portable/mod.rs).
- [Arguments and registry conversion](../src/api/portable/instruction.rs).
- [Structured boundary errors](../src/api/portable/error.rs).
- [Common tree and lowering](../src/api/builder/tree.rs).
- [Common structural policies](../src/parser/structure/rules.rs).
- [Existing processor execution](../src/api/processor.rs).
- [Instruction registry](../src/globals/table.rs).
