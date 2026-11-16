# TextForge FFI planning

The C FFI layer is planned and is not part of the current crate configuration (`Cargo.toml` currently exposes only `rlib`). Use this reference only for an explicitly requested FFI design or implementation.

- Add an explicit C-compatible API rather than exposing internal token types.
- Returned strings must follow a clear ownership contract: TextForge allocates and an exported TextForge free function releases them. `CString::into_raw` and `CString::from_raw` are the usual paired operations.
- Never unwind across an `extern "C"` boundary. Validate null pointers and convert `TextForgeError` failures to an agreed result/error representation.
- If enabling a dynamic library, evaluate the required `crate-type` change, symbol visibility, header generation, ABI tests, and language-binding packaging together. Do not imply that a future FFI design is already a stable public API.
