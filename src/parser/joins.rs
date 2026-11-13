//! `ppjn`: loading the pipelines it joins (see `docs/file-instructions.md`).
//!
//! A joined pipeline is loaded once, when the pipeline containing `ppjn` is
//! loaded (read from a file, built, registered), never while it runs. Its path
//! is resolved against the directory of the file containing `ppjn`, or the
//! working directory for pipelines that do not come from a file. Bytecode
//! embeds joined pipelines, so loading bytecode never reads other files.
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
    },
    text::reader::read_from_text,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// Maximum nesting of joined pipelines (`ppjn` inside a joined pipeline, and
/// so on). Applies to files and to embedded bytecode.
pub const MAX_JOIN_DEPTH: usize = 16;

/// The bytecode magic number, used to tell bytecode from text.
pub(crate) const BYTECODE_MAGIC: [u8; 8] = [38, 235, 245, 8, 244, 137, 1, 179];

/// A loaded pipeline joined by `ppjn`.
pub struct JoinedPipeline {
    origin: String,
    tokens: Vec<TokenWrapper>,
    /// Source line (1-based) of each instruction; its position for bytecode.
    lines: Vec<usize>,
}

impl JoinedPipeline {
    /// A pipeline embedded in bytecode, named after the path `ppjn` gives.
    #[cfg(feature = "bytecode")]
    pub(crate) fn embedded(tokens: Vec<TokenWrapper>) -> Self {
        let lines = (1..=tokens.len()).collect();
        JoinedPipeline {
            origin: String::new(),
            tokens: prepared(tokens),
            lines,
        }
    }

    pub(crate) fn tokens(&self) -> &[TokenWrapper] {
        &self.tokens
    }

    pub(crate) fn origin(&self) -> &str {
        &self.origin
    }

    /// Source line of instruction `index`.
    pub(crate) fn line(&self, index: usize) -> usize {
        self.lines.get(index).copied().unwrap_or(index + 1)
    }
}

/// Load state of a `ppjn` parameter.
#[derive(Clone)]
pub enum JoinState {
    /// Not loaded yet: the pipeline was not loaded through a path that links.
    Unloaded,
    Loaded(Arc<JoinedPipeline>),
}

impl std::fmt::Debug for JoinState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JoinState::Unloaded => f.write_str("Unloaded"),
            JoinState::Loaded(pipeline) => write!(f, "Loaded({})", pipeline.origin),
        }
    }
}

fn prepared(mut tokens: Vec<TokenWrapper>) -> Vec<TokenWrapper> {
    for token in &mut tokens {
        token.prepare_constant();
    }
    tokens
}

fn error(code: TextForgeErrorCode, detail: String) -> TextForgeError {
    TextForgeError::new(code, "ppjn", detail)
}

/// Report an error from inside a joined pipeline with its origin
/// (`ppjn lib.textforge:3 › …`), without quoting the file's contents.
pub(crate) fn in_join(mut error: TextForgeError, origin: &str, line: usize) -> TextForgeError {
    error.instruction = format!("ppjn {origin}:{line} › {}", error.instruction).into();
    error
}

/// Load every unloaded `ppjn` in `tokens`, nested ones included. `base` is
/// the directory relative paths start from; `chain` holds the files being
/// loaded, outermost first, for cycle detection.
pub(crate) fn link(
    tokens: &mut [TokenWrapper],
    base: &Path,
    chain: &[PathBuf],
) -> Result<(), TextForgeError> {
    for token in tokens {
        link_token(token, base, chain)?;
    }
    Ok(())
}

fn link_token(
    token: &mut TokenWrapper,
    base: &Path,
    chain: &[PathBuf],
) -> Result<(), TextForgeError> {
    let is_join = token.get_string_repr() == "ppjn";
    let mut path = None;
    for parameter in token.params_mut() {
        match parameter {
            ValType::Literal(TextForgeParamTypes::Token(child)) => link_token(child, base, chain)?,
            ValType::Literal(TextForgeParamTypes::String(text)) if is_join => {
                path = Some(text.clone());
            }
            ValType::Literal(TextForgeParamTypes::Pipeline(state @ JoinState::Unloaded)) => {
                let path = path.as_deref().ok_or_else(|| {
                    error(
                        TextForgeErrorCode::ValidationError("ppjn needs a literal path".into()),
                        String::new(),
                    )
                })?;
                *state = JoinState::Loaded(load(path, base, chain)?);
            }
            _ => {}
        }
    }
    Ok(())
}

/// Read, parse and link the pipeline at `path` (relative to `base`).
pub(crate) fn load(
    path: &str,
    base: &Path,
    chain: &[PathBuf],
) -> Result<Arc<JoinedPipeline>, TextForgeError> {
    let file = base.join(path);
    let canonical = file.canonicalize().map_err(|e| {
        error(
            TextForgeErrorCode::FileNotFound(e.to_string().into()),
            file.display().to_string(),
        )
    })?;
    if chain.contains(&canonical) {
        let names: Vec<_> = chain
            .iter()
            .chain([&canonical])
            .map(|path| path.display().to_string())
            .collect();
        return Err(error(
            TextForgeErrorCode::ValidationError("ppjn cycle".into()),
            names.join(" → "),
        ));
    }
    if chain.len() >= MAX_JOIN_DEPTH {
        return Err(error(
            TextForgeErrorCode::ValidationError(
                format!("ppjn nesting deeper than {MAX_JOIN_DEPTH}").into(),
            ),
            canonical.display().to_string(),
        ));
    }
    let bytes = std::fs::read(&canonical).map_err(|e| {
        error(
            TextForgeErrorCode::FileReadingError(e.to_string().into()),
            canonical.display().to_string(),
        )
    })?;
    let mut chain = chain.to_vec();
    chain.push(canonical.clone());
    let directory = canonical.parent().unwrap_or(Path::new("."));
    let origin = path.to_owned();

    let (mut tokens, lines) = if bytes.starts_with(&BYTECODE_MAGIC) {
        let tokens = read_bytecode(&bytes, &canonical)?;
        let lines = (1..=tokens.len()).collect();
        (tokens, lines)
    } else {
        parse_text(&bytes, &origin)?
    };
    for (token, line) in tokens.iter_mut().zip(&lines) {
        link_token(token, directory, &chain).map_err(|e| in_join(e, &origin, *line))?;
    }
    Ok(Arc::new(JoinedPipeline {
        origin,
        tokens: prepared(tokens),
        lines,
    }))
}

#[cfg(feature = "bytecode")]
fn read_bytecode(bytes: &[u8], _: &Path) -> Result<Vec<TokenWrapper>, TextForgeError> {
    crate::bytecode::reader::read_bytecode_from_bytes(bytes)
}

#[cfg(not(feature = "bytecode"))]
fn read_bytecode(_: &[u8], path: &Path) -> Result<Vec<TokenWrapper>, TextForgeError> {
    Err(error(
        TextForgeErrorCode::ValidationError(
            "ppjn: this is a bytecode pipeline; enable the `bytecode` feature".into(),
        ),
        path.display().to_string(),
    ))
}

fn parse_text(
    bytes: &[u8],
    origin: &str,
) -> Result<(Vec<TokenWrapper>, Vec<usize>), TextForgeError> {
    let text = std::str::from_utf8(bytes).map_err(|e| {
        error(
            TextForgeErrorCode::InvalidUtf8Error(e.to_string().into()),
            origin.to_owned(),
        )
    })?;
    let mut tokens = Vec::new();
    let mut lines = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }
        tokens.push(read_from_text(line).map_err(|e| in_join(e, origin, index + 1))?);
        lines.push(index + 1);
    }
    Ok((tokens, lines))
}

/// Parameters a syntax implies without text for them: the load state of a
/// joined pipeline. Added after the written ones by parsers and builders.
pub(crate) fn implicit_parameters(name: &str, parameters: &mut Vec<ValType>) {
    if name == "ppjn" && parameters.len() == 1 {
        parameters.push(ValType::Literal(TextForgeParamTypes::Pipeline(
            JoinState::Unloaded,
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(feature = "bytecode"))]
    #[test]
    fn joined_bytecode_needs_the_bytecode_feature() {
        let directory = std::env::temp_dir().join(format!("joins-test-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("compiled.textforgebc");
        std::fs::write(&path, BYTECODE_MAGIC).unwrap();
        let error = load("compiled.textforgebc", &directory, &[]).err().unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::ValidationError(_)
        ));
        std::fs::remove_dir_all(&directory).unwrap();
    }

    #[test]
    fn invalid_utf8_in_a_joined_text_file_is_an_error() {
        let directory = std::env::temp_dir().join(format!("joins-utf8-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("bad.textforge"), [b't', 0xFF]).unwrap();
        let error = load("bad.textforge", &directory, &[]).err().unwrap();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::InvalidUtf8Error(_)
        ));
        std::fs::remove_dir_all(&directory).unwrap();
    }
}
