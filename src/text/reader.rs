use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader},
    path::Path,
};

use crate::{
    globals::table::{QuerySource, QueryTarget, TOKEN_TABLE, TargetValue},
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    text::line_parser::split_line,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_file_path,
    },
};

pub fn read_from_text(token_string: &str) -> Result<TokenWrapper, TextForgeError> {
    let line = token_string.trim();

    if line.is_empty() || line.starts_with("//") {
        return Ok(TokenWrapper::default());
    }
    parse_line(line).map_err(|mut error| {
        if error.instruction.is_empty() {
            error.instruction = line.to_owned().into();
        }
        error
    })
}

/// Parse one non-empty, non-comment line; errors name no instruction, so the
/// caller can name the line.
fn parse_line(line: &str) -> Result<TokenWrapper, TextForgeError> {
    let body = line.trim_end().strip_suffix(';').ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::TextParsingError("an instruction must end with `;`".into()),
            "",
            "",
        )
    })?;
    let chunks = split_line(body)?;

    let identifier = chunks.first().ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::TextParsingError("the line has no instruction".into()),
            "",
            "",
        )
    })?;

    let token_query = TOKEN_TABLE.find((
        QuerySource::Identifier(identifier.clone().into()),
        QueryTarget::Token,
    ))?;

    let token_param_types = match TOKEN_TABLE.find((
        QuerySource::Identifier(identifier.clone().into()),
        QueryTarget::Syntax,
    ))? {
        TargetValue::Syntax(p) => p,
        _ => unreachable!(" Invalid Query result"),
    };

    match token_query {
        TargetValue::Token(token_ref) => {
            let token = token_ref.into_box();

            let parsed_params =
                TextForgeParamTypes::from_expected(token_param_types, &chunks[1..])?;

            let wrapper = TokenWrapper::new(token, Some(parsed_params));
            crate::parser::structure::validate_parsed(&wrapper)?;

            Ok(wrapper)
        }
        _ => unreachable!("Invalid query result!"),
    }
}

pub fn read_from_file(path: &Path) -> Result<Vec<TokenWrapper>, TextForgeError> {
    check_file_path(path, None)?;
    let mut result = Vec::new();

    let file = match OpenOptions::new().read(true).open(path) {
        Ok(x) => x,
        Err(_) => {
            return Err(TextForgeError::new(
                crate::utils::errors::TextForgeErrorCode::FileOpeningError(
                    "cannot open the pipeline file".into(),
                ),
                "",
                path.display().to_string(),
            ));
        }
    };

    let reader = BufReader::new(file);

    for (index, line) in reader.lines().enumerate() {
        match line {
            Ok(l) => {
                let trimmed = l.trim();
                if trimmed.is_empty() || trimmed.starts_with("//") {
                    continue;
                }
                result.push(read_from_text(&l).map_err(|mut error| {
                    error.instruction =
                        format!("{}:{} › {}", path.display(), index + 1, error.instruction).into();
                    error
                })?);
            }
            Err(error) => {
                return Err(TextForgeError::new(
                    crate::utils::errors::TextForgeErrorCode::FileReadingError(
                        format!(
                            "cannot read line {} of the pipeline file: {error}",
                            index + 1
                        )
                        .into(),
                    ),
                    "",
                    path.display().to_string(),
                ));
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_token_lists_without_panicking() {
        for source in [";", "  ;  ", "\t;", "\\\n;"] {
            let error = match read_from_text(source) {
                Err(error) => error,
                Ok(_) => panic!("expected an error for {source:?}"),
            };
            assert!(matches!(
                error.error_code,
                TextForgeErrorCode::TextParsingError(_)
            ));
        }
    }

    #[test]
    fn preserves_comments_and_valid_unicode_parameters() {
        for source in ["", "  ", "// comment", "null;", "atb \"👩🏽‍💻 é\";"] {
            assert!(read_from_text(source).is_ok(), "{source:?}");
        }
    }
}
