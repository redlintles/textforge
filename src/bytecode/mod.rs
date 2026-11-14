//! The `.textforgebc` binary format (`bytecode` feature).
//!
//! Pipelines are saved and loaded with
//! [`write_to_bytecode_file`](crate::api::processor::TextForgeProcessorMethods::write_to_bytecode_file)
//! and
//! [`read_from_bytecode_file`](crate::api::processor::TextForgeProcessorMethods::read_from_bytecode_file).
//! A file is an 8-byte magic number, a protocol version, an instruction count,
//! then one length-prefixed record per instruction (opcode and typed
//! parameters, big-endian). Joined pipelines (`ppjn`) are embedded, so a
//! bytecode file runs without the files it was built from.
#[doc(hidden)]
pub mod reader;
#[doc(hidden)]
pub mod writer;
