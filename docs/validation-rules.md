# Extending structural validation

The builder's internal hierarchy is validated before conversion or registration.
Rules are intentionally internal: applications use `processor.create_pipeline()`;
maintainers extend policy without exposing engine state or changing public traits.

## Components and responsibilities

| Component | Responsibility | Source |
| --- | --- | --- |
| `ValidationContext` | Inherited depth and whether any ancestor is a block | `src/parser/structure/rules.rs` |
| `Parameter` | Borrowed scalar, runtime reference, or instruction slot | `src/parser/structure/rules.rs` |
| `InstructionView` | Name and effective parameters of one real instruction/group | `src/parser/structure/rules.rs` |
| `ValidationRule` | Side-effect-free policy contract | `src/parser/structure/rules.rs` |
| `NestingRule` | Block ancestry and parser depth limits | `src/parser/structure/rules.rs` |
| `SignatureRule` | Registry arity and literal parameter types | `src/parser/structure/rules.rs` |
| `BUILTIN_RULES` | Ordered list of policies run at every visited node | `src/parser/structure/rules.rs` |

Each production structure/trait has a rustdoc example and links to related types.
Examples demonstrate the public operation governed by the internal structure;
applications do not need access to private validation internals.

The [instruction visitor](../src/parser/structure.rs) and
[construction-tree visitor](../src/api/builder/tree.rs) pass the same view/context
to these policies. The tree visitor presents group prefixes plus an instruction
slot; it never synthesizes a `null` child. Leaves can contain nested token wrappers,
which the instruction visitor checks with the inherited ancestor context.

## Add a rule

For example, a future policy could reject statically empty block names. This is
an illustrative extension, not an enabled policy in this release:

```rust
// Inside src/parser/structure/rules.rs:
struct NonEmptyBlockName;

impl ValidationRule for NonEmptyBlockName {
    fn check(
        &self,
        instruction: &InstructionView<'_>,
        _context: ValidationContext,
    ) -> Result<(), TextForgeError> {
        if instruction.name == "blk"
            && matches!(
                instruction.parameters.first(),
                Some(Parameter::Literal(TextForgeParamTypes::String(name)))
                    if name.is_empty()
            )
        {
            return Err(TextForgeError::new(
                TextForgeErrorCode::ValidationError("Empty block name".into()),
                instruction.name,
                "block name parameter",
            ));
        }
        Ok(())
    }
}
```

Then register it, documenting why the order matters:

```rust
const BUILTIN_RULES: &[&dyn ValidationRule] = &[
    &NestingRule,
    &SignatureRule,
    &NonEmptyBlockName,
];
```

No lowering or traversal change is needed. Add rustdoc immediately above the new
rule and regression tests alongside the existing
[extension tests](../src/parser/structure/test.rs). Those tests inject an extra
policy into the existing visitor and verify real descendants, literal access,
and inherited block/depth context. Test direct and nested cases, accepted input,
rejection diagnostics, and any references the rule must defer.

## Policy boundaries

Rules run in order and stop on the first error. Prefer nesting/signature checks
before rules that assume a particular parameter shape. Use `TextForgeError` and
include the instruction/parameter location in diagnostics. Do not execute tokens,
resolve variables, mutate the processor, or recursively walk children inside a
rule: the shared visitors already handle every descendant.

`Parameter::Reference` represents unresolved context-dependent input. A rule can
validate its structural position but cannot infer its future value. Instruction
slots carry no fictional child values; actual children receive separate visits.
Graph properties involving named block calls or conditional reachability need a
separate analysis design rather than hidden mutable state in a per-node rule.

Depth limits remain shared with the parsers. Changing a public method, opcode,
accepted syntax, or established depth policy still requires explicit human
approval under [AGENTS.md](../AGENTS.md).

## Useful references

- [Builder lifecycle and lowering](builder-validation.md).
- [Approved API migrations](api-migration.md).
- [Global instruction signatures](../src/globals/table.rs).
- [Text/bytecode parameter parsers and shared limits](../src/parser/params.rs).
- [Instruction trait](../src/tokens/mod.rs).
