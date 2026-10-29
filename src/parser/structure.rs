//! Structural checks shared by fluent builders and block parameter binding.
//!
//! Walk borrowed parameters with an explicit stack before cloning or flattening.
//! Keep the depth policy compatible with the textual and bytecode parsers.
use crate::{
    globals::table::{QuerySource, QueryTarget, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::{
        params::{MAX_DEPTH_ASSOC_PAYLOAD, MAX_DEPTH_NORMAL, TextForgeParamTypes},
        resolve_var::{TokenWrapper, ValType},
    },
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

pub(crate) fn validate(token: &TokenWrapper) -> Result<(), TextForgeError> {
    validate_at(token, 0, false)
}

pub(crate) fn validate_block_child(token: &TokenWrapper) -> Result<(), TextForgeError> {
    validate_at(token, 1, true)
}

pub(crate) fn validate_at(
    token: &TokenWrapper,
    depth: usize,
    inside_block: bool,
) -> Result<(), TextForgeError> {
    walk(vec![(token, depth, inside_block)])
}

fn walk(mut pending: Vec<(&TokenWrapper, usize, bool)>) -> Result<(), TextForgeError> {
    while let Some((token, depth, inside_block)) = pending.pop() {
        let name = token.get_string_repr();
        let is_block = name == "blk";
        if inside_block && is_block {
            return Err(TextForgeError::new(
                TextForgeErrorCode::NestedBlocksNotAllowedError(
                    "A block cannot contain another block at any descendant depth".into(),
                ),
                name,
                format!("depth={depth}"),
            ));
        }
        let max_depth = usize::from(if inside_block {
            MAX_DEPTH_ASSOC_PAYLOAD
        } else {
            MAX_DEPTH_NORMAL
        });
        if depth > max_depth {
            return Err(TextForgeError::new(
                TextForgeErrorCode::ValidationError("Nested token depth exceeded".into()),
                name,
                format!("depth={depth}, max={max_depth}"),
            ));
        }
        if let TargetValue::Syntax(syntax) =
            TOKEN_TABLE.find((QuerySource::Identifier(name.into()), QueryTarget::Syntax))?
        {
            let expected: Vec<_> = syntax
                .iter()
                .filter_map(|s| match s.token {
                    SyntaxToken::Literal(_) => None,
                    other => Some(other),
                })
                .collect();
            if expected.len() != token.get_params().len() {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters("Parameter count mismatch".into()),
                    name,
                    "builder structural validation",
                ));
            }
            for (value, expected) in token.get_params().iter().zip(expected) {
                // References are checked when their execution context is available.
                let ValType::Literal(value) = value else {
                    continue;
                };
                let matches = matches!(
                    (value, expected),
                    (TextForgeParamTypes::String(_), SyntaxToken::String)
                        | (TextForgeParamTypes::Usize(_), SyntaxToken::Usize)
                        | (TextForgeParamTypes::Token(_), SyntaxToken::Token)
                        | (TextForgeParamTypes::VarRef(_), _)
                );
                if !matches {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::IncompatibleTypeError(
                            "Literal parameter type mismatch".into(),
                        ),
                        name,
                        "builder structural validation",
                    ));
                }
                if let TextForgeParamTypes::Token(child) = value {
                    pending.push((child, depth + 1, inside_block || is_block));
                }
            }
        }
    }
    Ok(())
}
