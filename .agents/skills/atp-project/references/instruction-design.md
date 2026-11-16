# TextForge instruction design

Use this reference when adding an instruction or changing how an existing token is parsed, registered, serialized, or tested.

1. Inspect neighboring tokens with matching parameter shapes before choosing the mnemonic, struct fields, validation, or opcode.
2. Implement the actual `InstructionMethods` contract from `src/tokens/mod.rs`. `transform` takes and returns `Cow<'a, str>` (`Result<Cow<'a, str>, TextForgeError>`; return the borrowed input when nothing changes) and `set_params` validates `&[TextForgeParamTypes]`. Indices are zero-based character, word or line counts (`AGENTS.md` rules 21 to 24), and relative file paths go through `resolve_path` (rule 25).
3. Preserve the text representation (`to_textforge_line`), short identifier (`get_string_repr`), parsed parameter order, and bytecode parameter order as one coherent contract.
4. Add the entry in `src/globals/table.rs` (identifier, opcode, constructor and syntax; it is the authoritative registry), then the remaining steps of `AGENTS.md` section 12: `build_all_tokens_pipeline_safe`, a case in `tests/data/allocations/cases.json`, and the `CATALOG` of `scripts/instruction_index.py`, whose run writes the module lists and indexes.
5. Add focused tests in the adjacent `test.rs`: normal behavior, input/parameter boundary cases, Unicode behavior when indexing characters, and bytecode layout/round-trip behavior under `#[cfg(feature = "bytecode")]`.
6. For expected-success serialization in tests, call `to_bytecode().unwrap()` because the trait returns `Result<Vec<u8>, TextForgeError>`.

Mnemonic names generally use lowercase abbreviations. Confirm uniqueness in `TOKEN_TABLE` before selecting one. Prefer existing `TextForgeParamTypes` and validation helpers over creating a parallel parser.
