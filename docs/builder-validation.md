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

With explicit user approval, all built-in builder finalizers are now fallible:

- `TextForgeBuilder::build(&mut self) -> Result<String, TextForgeError>`.
- `BlockBuilder::build(self) -> Result<Vec<Box<dyn InstructionMethods>>, TextForgeError>`.
- `ConditionalBuilderEach::build(self) -> Result<Vec<Box<dyn InstructionMethods>>, TextForgeError>`.

The auxiliary builders store only their children. The old instruction caches,
compatibility projections, eager wrapping, and staging vectors have been removed.
Every finalizer uses the same validation/lowering implementation. Standalone
auxiliary finalizers bind only the outer instruction, retaining nested wrappers
and runtime references. Errors are reported at finalization instead of depending
on an eager compatibility cache.

Successful closures attach one group; failed closures attach none. A malformed
child makes the complete finalization fail before the processor is modified.
Third-party implementations of the public builder traits retain fallible default
methods that use the same validated auxiliary finalizers; the built-in builders
retain group nodes instead of flattening during collection.

See [API migration](api-migration.md) for the approved parameter-binding changes.

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
- `Blk::set_params` validates descendants before changing its state. Failed
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

The pre-commit checklist runs formatting, strict Clippy for default and
`test_access`, and tests for default, bytecode, watchers, and `test_access`. The
previous Clippy failures have been corrected without project-wide lint suppression.
