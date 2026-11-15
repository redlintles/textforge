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

    if is_blank_or_comment(line) {
        return Ok(TokenWrapper::default());
    }
    parse_line(line).map_err(|mut error| {
        if error.instruction.is_empty() {
            error.instruction = line.to_owned().into();
        }
        error
    })
}

/// Whether a trimmed line holds no instruction: empty, or a whole-line
/// comment starting with `//` or `#`.
fn is_blank_or_comment(line: &str) -> bool {
    line.is_empty() || line.starts_with("//") || line.starts_with('#')
}

/// `line` up to and including the `;` that ends its instruction, when what
/// follows is a comment (`ate x; // note`, `ate x; # note`). Any other line is
/// returned unchanged.
///
/// Only a `;` outside quotes that is followed by whitespace and `//` or `#`
/// ends the instruction, so `ate "a; // b";` and `ate a;b;` keep their meaning.
/// Lines that already end with `;` never reach this function, which keeps
/// every pipeline that parsed before 0.3.2 parsing the same way.
fn without_trailing_comment(line: &str) -> &str {
    let mut quote: Option<u8> = None;
    let mut escaped = false;
    for (index, byte) in line.bytes().enumerate() {
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, byte) {
            (Some(b'\''), b'\'') | (Some(b'"'), b'"') => quote = None,
            (Some(b'"'), b'\\') | (None, b'\\') => escaped = true,
            (None, b'\'' | b'"') => quote = Some(byte),
            (None, b';') => {
                let rest = line[index + 1..].trim_start();
                if rest.starts_with("//") || rest.starts_with('#') {
                    return &line[..=index];
                }
            }
            _ => {}
        }
    }
    line
}

/// Parse one non-empty, non-comment line; errors name no instruction, so the
/// caller can name the line.
fn parse_line(line: &str) -> Result<TokenWrapper, TextForgeError> {
    let line = line.trim_end();
    let line = if line.ends_with(';') {
        line
    } else {
        without_trailing_comment(line)
    };
    let body = line.strip_suffix(';').ok_or_else(|| {
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
                // Editors on Windows start UTF-8 files with a byte order mark.
                let l = match index {
                    0 => l.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(l),
                    _ => l,
                };
                if is_blank_or_comment(l.trim()) {
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

    /// Before 0.3.2 a comment after the instruction was an error
    /// ("an instruction must end with `;`").
    #[test]
    fn accepts_a_comment_after_the_instruction() {
        use crate::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
        for (source, expected) in [
            ("ate x; // note", "x"),
            ("ate x; # note", "x"),
            ("ate x;// note; more", "x"),
            ("ate 'a; // b'; // note", "a; // b"),
            ("ate \"a\\\"; # b\"; # note", "a\"; # b"),
            ("ate a;b;", "a;b"),
        ] {
            let token = read_from_text(source).unwrap_or_else(|e| panic!("{source:?}: {e:?}"));
            let mut processor = TextForgeProcessor::new();
            let id = processor.add_transform(vec![token]);
            assert_eq!(
                processor.process_all(&id, "").unwrap(),
                expected,
                "{source:?}"
            );
        }
        for source in ["# whole line", "#!/usr/bin/env txf"] {
            assert!(read_from_text(source).is_ok(), "{source:?}");
        }
        // A `;` is still required.
        assert!(read_from_text("ate x // note").is_err());
    }

    #[test]
    fn preserves_comments_and_valid_unicode_parameters() {
        for source in ["", "  ", "// comment", "null;", "atb \"👩🏽‍💻 é\";"] {
            assert!(read_from_text(source).is_ok(), "{source:?}");
        }
    }
}
