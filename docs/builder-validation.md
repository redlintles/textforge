# Tree-based pipeline construction

`TextForgeBuilder` owns a temporary hierarchy. Its root contains ordered nodes;
block and conditional nodes contain their own ordered children. Raw instruction
wrappers remain leaves and can contain nested token parameters. The execution
engine continues to own only `Vec<TokenWrapper>` and is unchanged.

## Construction, validation, lowering

1. Fluent calls append instructions or collect child builders inside a closure.
   A successful closure attaches one complete group node to its parent. A failed
   closure attaches nothing. The parent retains its previous valid children.
2. `build()` validates the complete construction tree, including nested token
   parameters inside leaves. An explicit work stack tracks depth and whether any
   ancestor is a block. No recursive tree walk consumes the Rust call stack.
3. Only after the entire validation pass succeeds does a second ordered traversal
   lower leaves, wrapping each leaf with its ancestors from the inside out.
4. Only after lowering succeeds is the vector registered with the processor. A
   failed build returns `TextForgeError` and neither registers a partial pipeline
   nor changes the builder. Successful builds retain the tree and can be repeated;
   each successful build registers a new ID, as before.

For example, the conceptual hierarchy

```text
root
├── block "cleanup"
│   ├── trim
│   └── contains_each "x"
│       ├── prepend "🍌"
│       └── append "é"
└── call "cleanup"
```

lowers to four entries:

```text
blk cleanup assoc tbs;
blk cleanup assoc ifdc x do atb 🍌;
blk cleanup assoc ifdc x do ate é;
cblk cleanup;
```

`contains_each` re-evaluates the condition for every child. It is not one shared
condition guarding a whole body. Moving nested instructions to independent root
entries would change this behavior; nesting within token parameters is retained.

## Compatibility and authorization

The only changed public method signature is the explicitly approved
`TextForgeBuilder::build(&mut self) -> Result<String, TextForgeError>`. Existing
fluent calls, closure types, trait availability, constructors, and auxiliary
`build()` signatures are retained. Callers must now propagate the build error or
unwrap it in a test. Rust examples and existing callers have been migrated.

The auxiliary builders retain validated instruction projections for their
existing standalone, infallible `build() -> Vec<Box<dyn InstructionMethods>>`
methods. These compatibility projections are prepared locally during collection;
the root builder ignores them and consumes the actual hierarchy. A block stages
an entire conditional projection before committing either representation, so a
late child error cannot partially append a group. Some local errors can still be
reported during collection; the root always performs complete final validation.

Third-party implementations of the public builder traits retain their existing
default methods. The tree and transactional group behavior described here apply
to the built-in builders; no new public trait requirement was introduced.

## Structural rules

- Traits still prohibit ordinary block definitions inside block or conditional
  bodies. Compile-fail doctests verify those boundaries.
- Raw `push_token` cannot bypass final validation: block declarations are rejected
  under any block ancestor, including beneath conditional or iteration tokens.
- The validator checks parameter counts, literal parameter types, and depth.
  Limits are shared with the existing parsers: root depth zero, normal maximum
  depth one, block-payload maximum depth three. No parser syntax or depth policy
  changed in this patch.
- References and interpolations depend on the runtime execution context. Static
  validation does not evaluate them, predict conditional branches, prove block
  call graphs acyclic, or replace semantic execution-time checks.
- `Blk::from_params` validates descendants before changing its state. Failed
  parameter binding preserves its previous instruction.

## Related correctness fixes

Conditional execution now calls `TokenWrapper::apply_token`, resolving the stored
parameters and variables rather than executing an unbound default template.

Text export uses an iterative post-order pass to bind children before parents.
Nested wrappers previously serialized default templates and lost parameters.
The unresolved bytecode persistence path retains nested variable references
instead of resolving them in an empty context. The existing public context-aware
parameter export methods still resolve values using the supplied context. The decoder accepts both existing size-prefixed token payloads
and opcode-first payloads used by existing parser tests. Emitted framing, endian
order, and the protocol version are unchanged.

With explicit user approval, `Val::get_opcode()` now emits `0x36`, matching the
registry. It previously emitted `0x38`, reserved for `null`, producing files the
reader rejected for incompatible parameter counts. No other opcode was changed;
old malformed `val` records are not reinterpreted as valid records. Regression
coverage checks the opcode against the registry and exercises file round trips.

## Verification and commits

The documentation-only commit fixes imports of the private `parser` module in
two doctests and repairs interrupted documentation comment blocks. Changes to
examples needed for the newly fallible `build()` belong to the implementation
commit alongside the corresponding call-site migrations.

Tests cover retained tree shape, repeated builds, failed closures, transactional
conditional insertion into blocks, direct and hidden descendant blocks, invalid
literal shapes, depth limits, Unicode, empty inputs, variable resolution,
condition re-evaluation, execution order, and text/bytecode round trips. The
round trip includes a `val` reference inside a block's conditional body.

All feature combinations and formatting are checked before committing. Strict
Clippy has existing failures outside this change, including public `&Vec`/`&Box`
signatures and naming conventions. They are reported rather than changing
additional public contracts or silencing lints across the project.
