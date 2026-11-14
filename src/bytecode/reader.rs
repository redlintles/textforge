use std::{
    cell::Cell,
    fs::OpenOptions,
    io::{BufReader, Cursor, Read, Seek, SeekFrom},
    path::Path,
};

use crate::{
    globals::table::{QuerySource, QueryTarget, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
    },
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        expr::is_numeric_template,
        validations::check_file_path,
    },
};

const PARAM_STRING: u32 = 0x01;
const PARAM_USIZE: u32 = 0x02;
const PARAM_TOKEN: u32 = 0x03;
const PARAM_VARREF: u32 = 0x04;
use crate::parser::params::PARAM_PIPELINE;

/// Where bytecode is read from: a file, or an embedded pipeline in memory.
trait Source: Read + Seek {
    /// Bytes left to read, to check declared sizes before trusting them.
    fn remaining(&mut self) -> std::io::Result<u64>;
}

impl Source for BufReader<std::fs::File> {
    fn remaining(&mut self) -> std::io::Result<u64> {
        let length = self.get_ref().metadata()?.len();
        Ok(length.saturating_sub(self.stream_position()?))
    }
}

impl Source for Cursor<&[u8]> {
    fn remaining(&mut self) -> std::io::Result<u64> {
        Ok((self.get_ref().len() as u64).saturating_sub(self.position()))
    }
}

fn param_type_from_code(code: u32) -> Option<SyntaxToken> {
    match code {
        PARAM_STRING => Some(SyntaxToken::String),
        PARAM_USIZE => Some(SyntaxToken::Usize),
        PARAM_TOKEN => Some(SyntaxToken::Token),
        PARAM_VARREF => Some(SyntaxToken::String), // VarRef ocupa slot "string-like" no schema
        PARAM_PIPELINE => Some(SyntaxToken::Pipeline),
        _ => None,
    }
}

/// Whether a parameter of type `actual` may fill a slot that expects `expected`.
/// A variable reference (0x04) is accepted wherever a string, number or
/// instruction is expected; its value is checked when it is resolved at run time.
fn bytecode_compatible(expected: &SyntaxToken, actual: &SyntaxToken, type_code: u32) -> bool {
    match (expected, actual, type_code) {
        // Direct types
        (SyntaxToken::String, SyntaxToken::String, PARAM_STRING) => true,
        (SyntaxToken::Usize, SyntaxToken::Usize, PARAM_USIZE) => true,
        // Numeric expression stored as text; its payload is checked after decoding.
        (SyntaxToken::Usize, SyntaxToken::String, PARAM_STRING) => true,
        (SyntaxToken::Token, SyntaxToken::Token, PARAM_TOKEN) => true,
        (SyntaxToken::Pipeline, SyntaxToken::Pipeline, PARAM_PIPELINE) => true,

        // A VarRef can appear wherever the schema expects String/Usize/Token
        (SyntaxToken::String, SyntaxToken::String, PARAM_VARREF) => true,
        (SyntaxToken::Usize, SyntaxToken::String, PARAM_VARREF) => true,
        (SyntaxToken::Token, SyntaxToken::String, PARAM_VARREF) => true,

        _ => false,
    }
}

/// Reads exactly `N` bytes, reporting a truncated file as a bytecode error.
fn read_exact<const N: usize>(
    reader: &mut impl Source,
    ctx: &'static str,
) -> Result<[u8; N], TextForgeError> {
    let mut buf = [0u8; N];
    reader.read_exact(&mut buf).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("the bytecode ended unexpectedly; the file is corrupt or was not written by TextForge".into()),
            ctx,
            e.to_string(),
        )
    })?;
    Ok(buf)
}

/// Check the physical file bound before trusting a declared byte count.
fn ensure_available(
    reader: &mut impl Source,
    len: u64,
    ctx: &'static str,
) -> Result<(), TextForgeError> {
    let remaining = reader.remaining().map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(
                "cannot determine the size of the bytecode file".into(),
            ),
            ctx,
            error.to_string(),
        )
    })?;
    if len > remaining {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(
                format!("a record declares {len} bytes, but only {remaining} remain; the file is truncated or corrupt").into(),
            ),
            ctx,
            format!("declared={len}, remaining={remaining}"),
        ));
    }
    Ok(())
}

/// Reserve fallibly so malformed sizes cannot trigger an allocator abort.
fn byte_buffer(len: usize, ctx: &'static str) -> Result<Vec<u8>, TextForgeError> {
    let mut buffer = Vec::new();
    buffer.try_reserve_exact(len).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(
                "not enough memory for a bytecode record".into(),
            ),
            ctx,
            error.to_string(),
        )
    })?;
    Ok(buffer)
}

fn read_vec(
    reader: &mut impl Source,
    len: usize,
    ctx: &'static str,
) -> Result<Vec<u8>, TextForgeError> {
    ensure_available(reader, len as u64, ctx)?;
    let mut buf = byte_buffer(len, ctx)?;
    buf.resize(len, 0);
    reader.read_exact(&mut buf).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("the bytecode ended unexpectedly; the file is corrupt or was not written by TextForge".into()),
            ctx,
            e.to_string(),
        )
    })?;
    Ok(buf)
}

/// Skip compatible instruction padding without allocating a padding-sized buffer.
fn skip_bytes(reader: &mut impl Source, len: u64) -> Result<(), TextForgeError> {
    const CONTEXT: &str = "bytecode instruction";
    ensure_available(reader, len, CONTEXT)?;
    reader
        .stream_position()
        .and_then(|position| reader.seek(SeekFrom::Start(position + len)))
        .map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "cannot skip the padding of a bytecode record".into(),
                ),
                CONTEXT,
                error.to_string(),
            )
        })?;
    Ok(())
}

/// Decodes UTF-8 bytes, reporting invalid UTF-8 as a parameter error.
fn utf8_string(bytes: &[u8], ctx: &'static str) -> Result<String, TextForgeError> {
    std::str::from_utf8(bytes)
        .map(|s| s.to_string())
        .map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParamParsingError(
                    "a text parameter in the bytecode is not valid UTF-8".into(),
                ),
                ctx,
                e.to_string(),
            )
        })
}

/// Decodes one parameter record (without its outer `u64` size) into a literal
/// or a variable reference.
fn decode_param_record_to_valtype(
    param_record: &[u8],
    opcode: u32,
) -> Result<ValType, TextForgeError> {
    if param_record.len() < 8 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("a parameter record is shorter than its header; the file is corrupt or was not written by TextForge".into()),
            "bytecode parameter",
            format!("len={}", param_record.len()),
        ));
    }

    let type_code = u32::from_be_bytes([
        param_record[0],
        param_record[1],
        param_record[2],
        param_record[3],
    ]);
    let payload_size = u32::from_be_bytes([
        param_record[4],
        param_record[5],
        param_record[6],
        param_record[7],
    ]) as usize;

    let remaining = param_record.len().saturating_sub(8);
    if payload_size > remaining {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("a parameter declares more bytes than its record holds; the file is corrupt or was not written by TextForge".into()),
            "bytecode parameter",
            format!("payload_size={}, remaining={}", payload_size, remaining),
        ));
    }

    let payload = &param_record[8..8 + payload_size];

    match type_code {
        PARAM_STRING => {
            let s = utf8_string(payload, "bytecode parameter")?;
            Ok(ValType::Literal(TextForgeParamTypes::String(s)))
        }
        PARAM_USIZE => {
            if payload.len() != 8 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParamParsingError(
                        "a number parameter must be 8 bytes long; the file is corrupt or was not written by TextForge".into(),
                    ),
                    "bytecode parameter",
                    format!("len={}", payload.len()),
                ));
            }
            let mut b = [0u8; 8];
            b.copy_from_slice(payload);
            Ok(ValType::Literal(TextForgeParamTypes::Usize(
                usize::from_be_bytes(b),
            )))
        }
        PARAM_VARREF => {
            let name = utf8_string(payload, "bytecode parameter")?;
            Ok(ValType::VarRef(name))
        }
        PARAM_TOKEN => {
            // Here the payload is the "instruction payload":
            // opcode(u32) + param_count(u8) + params...
            // TextForgeParamTypes::from_bytecode reads the PARAM_TOKEN layout
            // (it expects the complete parameter record, total_size first,
            // so rebuild the "complete parameter": total(u64)+type+payload_size+payload.
            let total = 8u64 + 4 + 4 + (payload.len() as u64);
            let full_len = usize::try_from(total).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "a nested instruction is too large to read on this machine".into(),
                    ),
                    "bytecode parameter",
                    error.to_string(),
                )
            })?;
            let mut full_param =
                byte_buffer(full_len, "bytecode parameter")?;
            full_param.extend_from_slice(&total.to_be_bytes());
            full_param.extend_from_slice(&PARAM_TOKEN.to_be_bytes());
            full_param.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            full_param.extend_from_slice(payload);

            let parsed = TextForgeParamTypes::from_bytecode_as_root_child(&full_param, opcode)?;
            match parsed {
                TextForgeParamTypes::Token(tw) => {
                    Ok(ValType::Literal(TextForgeParamTypes::Token(tw)))
                }
                _ => Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "internal error: a nested-instruction parameter did not decode to an instruction".into(),
                    ),
                    "bytecode parameter",
                    "",
                )),
            }
        }
        PARAM_PIPELINE => Ok(ValType::Literal(TextForgeParamTypes::Pipeline(
            decode_pipeline_payload(payload)?,
        ))),
        _ => Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParamNotRecognized(
                format!("unknown parameter type 0x{type_code:02X} (known types: 0x01 text, 0x02 number, 0x03 instruction, 0x04 variable, 0x05 joined pipeline)").into(),
            ),
            "bytecode parameter",
            "",
        )),
    }
}

pub fn read_bytecode_from_file(path: &Path) -> Result<Vec<TokenWrapper>, TextForgeError> {
    check_file_path(path, Some("textforgebc"))?;

    let file = OpenOptions::new().read(true).open(path).map_err(|_| {
        TextForgeError::new(
            TextForgeErrorCode::FileOpeningError("cannot open the bytecode file".into()),
            "bytecode file",
            format!("{:?}", path),
        )
    })?;

    read_file(file, path, IN_MEMORY_LIMIT)
}

/// Pipelines are small: decoding from memory avoids the length and position
/// system calls each bounds check makes on a file. Files over `limit` bytes are
/// still streamed, so memory stays bounded.
fn read_file(
    file: std::fs::File,
    path: &Path,
    limit: u64,
) -> Result<Vec<TokenWrapper>, TextForgeError> {
    let length = file
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(u64::MAX);
    if length <= limit {
        let mut bytes = Vec::with_capacity(length as usize);
        let mut file = file;
        file.read_to_end(&mut bytes).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError("cannot read the bytecode file".into()),
                "bytecode file",
                format!("{:?}", path),
            )
        })?;
        return read_bytecode_from_bytes(&bytes);
    }
    read_pipeline(&mut BufReader::new(file))
}

/// Largest bytecode file decoded from memory instead of streamed.
const IN_MEMORY_LIMIT: u64 = 64 * 1024 * 1024;

/// Read a complete bytecode pipeline held in memory (a file `ppjn` joins).
pub(crate) fn read_bytecode_from_bytes(bytes: &[u8]) -> Result<Vec<TokenWrapper>, TextForgeError> {
    read_pipeline(&mut Cursor::new(bytes))
}

/// Header, then instructions.
fn read_pipeline(reader: &mut impl Source) -> Result<Vec<TokenWrapper>, TextForgeError> {
    // --- header ---
    let expected_magic_number: [u8; 8] = [38, 235, 245, 8, 244, 137, 1, 179];
    let magic_number = read_exact::<8>(reader, "bytecode header")?;
    if magic_number != expected_magic_number {
        return Err(TextForgeError::new(
            TextForgeErrorCode::FileReadingError("this is not a TextForge bytecode file: it does not start with the TextForge magic number".into()),
            "bytecode file",
            "",
        ));
    }

    let protocol_version_bytes = read_exact::<8>(reader, "bytecode header")?;
    let protocol_version = u64::from_be_bytes(protocol_version_bytes);
    if protocol_version != 1 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(format!("unsupported bytecode protocol version {protocol_version}; this version of TextForge reads version 1").into()),
            "bytecode file",
            protocol_version.to_string(),
        ));
    }

    let instruction_count_bytes = read_exact::<4>(reader, "bytecode header")?;
    let instruction_count = u32::from_be_bytes(instruction_count_bytes);

    // Even a parameterless instruction needs size(8), opcode(4), count(1).
    // Validate plausibility without reserving memory from an untrusted count.
    ensure_available(reader, u64::from(instruction_count) * 13, "bytecode header")?;
    read_instructions(reader, instruction_count)
}

thread_local! {
    /// Embedded pipelines currently being decoded, to bound recursion.
    static EMBEDDED_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Decode an embedded joined pipeline (parameter type 0x05): instruction count
/// (u32), then instruction records. Nesting is limited to
/// [`MAX_JOIN_DEPTH`](crate::parser::joins::MAX_JOIN_DEPTH).
pub(crate) fn decode_pipeline_payload(
    payload: &[u8],
) -> Result<crate::parser::joins::JoinState, TextForgeError> {
    use crate::parser::joins::{JoinState, JoinedPipeline, MAX_JOIN_DEPTH};
    const CONTEXT: &str = "bytecode ppjn pipeline";
    let depth = EMBEDDED_DEPTH.with(Cell::get);
    if depth >= MAX_JOIN_DEPTH {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError(
                format!(
                    "pipelines joined with ppjn are nested more than {MAX_JOIN_DEPTH} levels deep"
                )
                .into(),
            ),
            CONTEXT,
            format!("max={MAX_JOIN_DEPTH}"),
        ));
    }
    let mut reader = Cursor::new(payload);
    let count = u32::from_be_bytes(read_exact::<4>(&mut reader, CONTEXT)?);
    EMBEDDED_DEPTH.with(|cell| cell.set(depth + 1));
    let tokens = read_instructions(&mut reader, count);
    EMBEDDED_DEPTH.with(|cell| cell.set(depth));
    let tokens = tokens?;
    if reader.remaining().unwrap_or(0) != 0 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::BytecodeParsingError("an embedded ppjn pipeline has extra bytes after its last instruction; the file is corrupt or was not written by TextForge".into()),
            CONTEXT,
            "",
        ));
    }
    Ok(JoinState::Loaded(std::sync::Arc::new(
        JoinedPipeline::embedded(tokens),
    )))
}

/// `count` instruction records.
fn read_instructions(
    reader: &mut impl Source,
    instruction_count: u32,
) -> Result<Vec<TokenWrapper>, TextForgeError> {
    let mut result: Vec<TokenWrapper> = Vec::new();

    // --- body ---
    for _ in 0..instruction_count {
        // instruction_total_size (u64)
        let instruction_total_size_bytes = read_exact::<8>(reader, "bytecode instruction")?;
        let instruction_total_size = u64::from_be_bytes(instruction_total_size_bytes);

        // internal minimum = opcode(4) + param_count(1) = 5
        if instruction_total_size < 5 {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("an instruction record declares an impossible size; the file is corrupt or was not written by TextForge".into()),
                "bytecode file",
                format!("instruction_total_size={}", instruction_total_size),
            ));
        }

        ensure_available(reader, instruction_total_size, "bytecode instruction")?;

        // bytes consumed inside the instruction (not counting the 8 of total_size)
        let mut consumed_in_instruction: u64 = 0;

        // opcode (u32)
        let opcode_bytes = read_exact::<4>(reader, "bytecode instruction")?;
        let opcode = u32::from_be_bytes(opcode_bytes);
        consumed_in_instruction += 4;

        if opcode == 0 {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError("an instruction has opcode 0, which no instruction uses; the file is corrupt or was not written by TextForge".into()),
                "bytecode file",
                "opcode=0".to_string(),
            ));
        }

        // param_count (u8)
        let param_count_bytes = read_exact::<1>(reader, "bytecode instruction")?;
        let param_count = param_count_bytes[0] as usize;
        consumed_in_instruction += 1;

        // schema
        let expected =
            match TOKEN_TABLE.find((QuerySource::Bytecode(opcode), QueryTarget::Syntax))? {
                TargetValue::Syntax(p) => p,
                _ => unreachable!("Invalid query result (Syntax)"),
            };

        let min_required = expected
            .iter()
            .filter(|p| !p.optional)
            .filter(|p| !matches!(p.token, SyntaxToken::Literal(_)))
            .count();
        let max_allowed = expected
            .iter()
            .filter(|p| !matches!(p.token, SyntaxToken::Literal(_)))
            .count();

        if param_count < min_required || param_count > max_allowed {
            return Err(TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "an instruction has the wrong number of parameters for its opcode".into(),
                ),
                "bytecode file",
                format!(
                    "opcode=0x{:x} got={} expected=[min={}, max={}]",
                    opcode, param_count, min_required, max_allowed
                ),
            ));
        }

        // read the params as ValType and check them against the schema
        let mut params: Vec<ValType> = Vec::with_capacity(param_count);
        let mut expected_i = 0usize;

        for param_i in 0..param_count {
            // Param total size (u64)
            let param_total_size_bytes = read_exact::<8>(reader, "bytecode instruction")?;
            let param_total_size = u64::from_be_bytes(param_total_size_bytes);
            consumed_in_instruction += 8;

            if param_total_size < 8 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("a parameter record declares an impossible size; the file is corrupt or was not written by TextForge".into()),
                    "bytecode file",
                    format!("param_total_size={}", param_total_size),
                ));
            }

            // A parameter record must also fit inside its enclosing instruction.
            let record_size = param_total_size - 8;
            if record_size > instruction_total_size.saturating_sub(consumed_in_instruction) {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "a parameter is larger than the instruction that contains it; the file is corrupt or was not written by TextForge".into(),
                    ),
                    "bytecode instruction",
                    format!("param_total_size={param_total_size}"),
                ));
            }
            let param_record_len = usize::try_from(record_size).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "a parameter is too large to read on this machine".into(),
                    ),
                    "bytecode instruction",
                    error.to_string(),
                )
            })?;
            let param_record = read_vec(reader, param_record_len, "bytecode instruction")?;
            consumed_in_instruction += param_record_len as u64;

            if param_record.len() < 4 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError("a parameter record is shorter than its header; the file is corrupt or was not written by TextForge".into()),
                    "bytecode file",
                    format!("opcode=0x{:x} param_index={}", opcode, param_i),
                ));
            }

            let type_code = u32::from_be_bytes([
                param_record[0],
                param_record[1],
                param_record[2],
                param_record[3],
            ]);
            let actual_pt = param_type_from_code(type_code).ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::BytecodeParamNotRecognized(
                        "unknown parameter type (known types: 0x01 text, 0x02 number, 0x03 instruction, 0x04 variable, 0x05 joined pipeline)".into(),
                    ),
                    "bytecode file",
                    format!("type_code=0x{:x}", type_code),
                )
            })?;

            // advance the schema, skipping Literals and optional slots when needed
            while expected_i < expected.len() {
                let expected_pt = &expected[expected_i].token;

                if matches!(expected_pt, SyntaxToken::Literal(_)) {
                    expected_i += 1;
                    continue;
                }

                if bytecode_compatible(expected_pt, &actual_pt, type_code) {
                    break;
                }

                if expected[expected_i].optional {
                    expected_i += 1;
                    continue;
                }

                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "a parameter has the wrong type for its instruction".into(),
                    ),
                    "bytecode file",
                    format!(
                        "opcode=0x{:x} param={} got={:?}(0x{:x}) expected={:?}",
                        opcode, param_i, actual_pt, type_code, expected_pt
                    ),
                ));
            }

            if expected_i >= expected.len() {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "an instruction has more parameters than its opcode accepts".into(),
                    ),
                    "bytecode file",
                    format!("opcode=0x{:x} param_index={}", opcode, param_i),
                ));
            }

            let vt = decode_param_record_to_valtype(&param_record, opcode)?;
            if matches!(expected[expected_i].token, SyntaxToken::Usize)
                && matches!(&vt, ValType::Literal(TextForgeParamTypes::String(text)) if !is_numeric_template(text))
            {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "a parameter has the wrong type for its instruction".into(),
                    ),
                    "bytecode file",
                    format!("opcode=0x{opcode:x} param={param_i} got=String expected=Usize"),
                ));
            }
            params.push(vt);
            expected_i += 1;
        }

        // guardrail instruction_total_size
        if consumed_in_instruction != instruction_total_size {
            if consumed_in_instruction < instruction_total_size {
                skip_bytes(reader, instruction_total_size - consumed_in_instruction)?;
            } else {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::BytecodeParsingError(
                        "the parameters of an instruction do not match its declared size; the file is corrupt or was not written by TextForge".into(),
                    ),
                    "bytecode file",
                    format!(
                        "opcode=0x{:x} consumed={} expected={}",
                        opcode, consumed_in_instruction, instruction_total_size
                    ),
                ));
            }
        }

        // create the default instruction (no params applied!)
        let token_ref =
            match TOKEN_TABLE.find((QuerySource::Bytecode(opcode), QueryTarget::Token))? {
                TargetValue::Token(t) => t,
                _ => unreachable!("Invalid query result (Token)"),
            };
        let token = token_ref.into_box();

        // Grow only for records actually decoded and report reservation failures.
        result.try_reserve(1).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "not enough memory for the number of instructions the file declares".into(),
                ),
                "bytecode header",
                error.to_string(),
            )
        })?;
        let wrapper = TokenWrapper::new(token, Some(params));
        crate::parser::structure::validate_parsed(&wrapper)?;
        result.push(wrapper);
    }

    Ok(result)
}

#[cfg(all(test, feature = "test_access"))]
mod tests {
    use super::*;

    fn header(count: u32) -> Vec<u8> {
        let mut bytes = vec![38, 235, 245, 8, 244, 137, 1, 179];
        bytes.extend_from_slice(&1u64.to_be_bytes());
        bytes.extend_from_slice(&count.to_be_bytes());
        bytes
    }

    fn parse(bytes: &[u8]) -> Result<Vec<TokenWrapper>, TextForgeError> {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pipeline.textforgebc");
        std::fs::write(&path, bytes).unwrap();
        read_bytecode_from_file(&path)
    }

    #[test]
    fn rejects_maximum_instruction_count_in_tiny_file() {
        let error = parse(&header(u32::MAX)).err().unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
        assert_eq!(error.instruction, "bytecode header");
    }

    #[test]
    fn rejects_count_exceeding_available_records() {
        let mut bytes = header(2);
        bytes.extend_from_slice(&5u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn empty_pipeline_remains_valid() {
        assert!(parse(&header(0)).unwrap().is_empty());
    }

    #[test]
    fn rejects_maximum_instruction_size_before_allocation() {
        let mut bytes = header(1);
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert!(matches!(
            parse(&bytes).err().unwrap().error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
    }

    #[test]
    fn rejects_parameter_size_exceeding_instruction() {
        let mut bytes = header(1);
        bytes.extend_from_slice(&21u64.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&u64::MAX.to_be_bytes());
        bytes.extend_from_slice(&PARAM_STRING.to_be_bytes());
        bytes.extend_from_slice(&0u32.to_be_bytes());
        assert!(matches!(
            parse(&bytes).err().unwrap().error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
    }

    /// One instruction record: size, opcode, parameters as (type, payload).
    fn record(opcode: u32, parameters: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut body = opcode.to_be_bytes().to_vec();
        body.push(parameters.len() as u8);
        for (kind, payload) in parameters {
            body.extend_from_slice(&(16 + payload.len() as u64).to_be_bytes());
            body.extend_from_slice(&kind.to_be_bytes());
            body.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            body.extend_from_slice(payload);
        }
        let mut bytes = (body.len() as u64).to_be_bytes().to_vec();
        bytes.extend_from_slice(&body);
        bytes
    }

    /// `ppjn` records nested `levels` deep around a `null`.
    fn nested_joins(levels: usize) -> Vec<u8> {
        let mut inner = record(0x38, &[]);
        for _ in 0..levels {
            let mut payload = 1u32.to_be_bytes().to_vec();
            payload.extend_from_slice(&inner);
            inner = record(
                0x58,
                &[
                    (PARAM_STRING, b"lib.textforge".to_vec()),
                    (PARAM_PIPELINE, payload),
                ],
            );
        }
        let mut bytes = header(1);
        bytes.extend_from_slice(&inner);
        bytes
    }

    #[test]
    fn embedded_joins_are_decoded_up_to_the_nesting_limit() {
        use crate::parser::joins::MAX_JOIN_DEPTH;
        let tokens = read_bytecode_from_bytes(&nested_joins(MAX_JOIN_DEPTH)).unwrap();
        assert_eq!(tokens.len(), 1);
        let error = read_bytecode_from_bytes(&nested_joins(MAX_JOIN_DEPTH + 1))
            .err()
            .unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::BytecodeParsingError(_)
        ));
        // Deep malicious nesting fails cleanly instead of exhausting the stack.
        assert!(read_bytecode_from_bytes(&nested_joins(5_000)).is_err());
    }

    #[test]
    fn embedded_joins_reject_trailing_bytes() {
        let mut payload = 1u32.to_be_bytes().to_vec();
        payload.extend_from_slice(&record(0x38, &[]));
        payload.push(0);
        let mut bytes = header(1);
        bytes.extend_from_slice(&record(
            0x58,
            &[(PARAM_STRING, b"x".to_vec()), (PARAM_PIPELINE, payload)],
        ));
        assert!(read_bytecode_from_bytes(&bytes).is_err());
    }

    #[test]
    fn preserves_padding_and_next_instruction() {
        let mut bytes = header(2);
        bytes.extend_from_slice(&8u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.extend_from_slice(&[0, 7, 8, 9]);
        bytes.extend_from_slice(&5u64.to_be_bytes());
        bytes.extend_from_slice(&0x38u32.to_be_bytes());
        bytes.push(0);
        assert_eq!(parse(&bytes).unwrap().len(), 2);
    }

    #[test]
    fn streamed_and_in_memory_files_decode_alike() {
        let mut bytes = header(2);
        for line in ["atb '!';", "rfw a 'b c';"] {
            let token = crate::text::reader::read_from_text(line).unwrap();
            bytes.extend(token.to_bytecode_unresolved().unwrap());
        }
        let path =
            std::env::temp_dir().join(format!("tf-stream-{}.textforgebc", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let lines = |limit| {
            let file = std::fs::File::open(&path).unwrap();
            read_file(file, &path, limit)
                .unwrap()
                .iter()
                .map(|token| token.to_text_line_unresolved().unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(lines(0), lines(u64::MAX));
        assert_eq!(lines(0).len(), 2);
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn unicode_parameters_still_roundtrip() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("pipeline.textforgebc");
        std::fs::write(&path, []).unwrap();
        let token = crate::text::reader::read_from_text("atb \"👩🏽‍💻 é\";").unwrap();
        crate::bytecode::writer::write_bytecode_to_file(&path, vec![token]).unwrap();
        let tokens = read_bytecode_from_file(&path).unwrap();
        let mut context = crate::context::execution_context::GlobalExecutionContext::new();
        assert_eq!(
            tokens[0].apply_token("x".into(), &mut context).unwrap(),
            "👩🏽‍💻 éx"
        );
    }
}
