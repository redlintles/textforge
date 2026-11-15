//! Lexer for one instruction line of a `.textforge` pipeline.
//!
//! Replaces `shell_words::split`/`shell_words::quote`. The rules are the
//! POSIX-shell rules that `shell_words` implemented, so existing pipelines
//! keep their meaning, plus control-character escapes inside double quotes:
//!
//! | Written | Unquoted | `'single'` | `"double"` |
//! |---|---|---|---|
//! | `\n` `\t` `\r` `\0` | error | kept as written | newline, tab, carriage return, NUL |
//! | `\\` `\"` `\$` `` \` `` | the second character | kept as written | the second character |
//! | `\` + any other character | the second character | kept as written | kept as written (`"\d"` is `\d`) |
//! | `#` at the start of a word | comment until the end of the line | literal | literal |
//! | a real line break | error | error | error |
//!
//! An instruction is one line: a real line break anywhere is an error, and so
//! is `\n` outside double quotes, which a reader would take as one.

use std::borrow::Cow;

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// Splits one instruction line (without its trailing `;`) into words.
///
/// Scans the line once. Runs of ordinary characters are copied as slices, so
/// the cost is one allocation per word. The rules are in the
/// [module documentation](self).
///
/// # Errors
///
/// `TextParsingError` for an unterminated quote, a real line break, an escape
/// at the end of a double-quoted word, or a control escape (`\n`, `\t`, `\r`,
/// `\0`) outside double quotes.
///
/// # Example
///
/// ```
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let path = std::env::temp_dir().join("textforge_doc_split_line.textforge");
/// // Double quotes: \n is a newline. Single quotes: literal.
/// std::fs::write(&path, "raw \" \" \"\\n\";\natb 'a\\nb ';\n").unwrap();
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.get_pipeline_from_file(&path).unwrap();
/// assert_eq!(processor.process_all(&id, "x y").unwrap(), "a\\nb x\ny");
///
/// // \n outside double quotes is an error.
/// std::fs::write(&path, "atb banana\\nlaranja;\n").unwrap();
/// assert!(processor.get_pipeline_from_file(&path).is_err());
/// # std::fs::remove_file(&path).unwrap();
/// ```
pub fn split_line(line: &str) -> Result<Vec<String>, TextForgeError> {
    let bytes = line.as_bytes();
    let mut words = Vec::new();
    let mut word = String::new();
    // True once the current word has started, so `""` yields an empty word.
    let mut in_word = false;
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
                i += 1;
            }
            b'\n' | b'\r' => return Err(line_break_error()),
            b'#' if !in_word => break,
            b'\'' => {
                in_word = true;
                i = single_quoted(line, i + 1, &mut word)?;
            }
            b'"' => {
                in_word = true;
                i = double_quoted(line, i + 1, &mut word)?;
            }
            b'\\' => {
                in_word = true;
                i = unquoted_escape(line, i + 1, &mut word)?;
            }
            _ => {
                in_word = true;
                let end = bytes[i..]
                    .iter()
                    .position(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'\'' | b'"' | b'\\'))
                    .map_or(bytes.len(), |offset| i + offset);
                // `end` stops at an ASCII byte, so it is a char boundary.
                word.push_str(&line[i..end]);
                i = end;
            }
        }
    }

    if in_word {
        words.push(word);
    }
    Ok(words)
}

/// Copies a single-quoted run starting at `start` (after the opening quote)
/// and returns the index after the closing quote.
fn single_quoted(line: &str, start: usize, word: &mut String) -> Result<usize, TextForgeError> {
    let rest = &line.as_bytes()[start..];
    let end = rest
        .iter()
        .position(|&b| matches!(b, b'\'' | b'\n' | b'\r'))
        .ok_or_else(unterminated_error)?;
    if rest[end] != b'\'' {
        return Err(line_break_error());
    }
    word.push_str(&line[start..start + end]);
    Ok(start + end + 1)
}

/// Copies a double-quoted run starting at `start` (after the opening quote),
/// resolving escapes, and returns the index after the closing quote.
fn double_quoted(line: &str, start: usize, word: &mut String) -> Result<usize, TextForgeError> {
    let bytes = line.as_bytes();
    let mut i = start;
    loop {
        let offset = bytes[i..]
            .iter()
            .position(|&b| matches!(b, b'"' | b'\\' | b'\n' | b'\r'))
            .ok_or_else(unterminated_error)?;
        word.push_str(&line[i..i + offset]);
        i += offset;
        match bytes[i] {
            b'"' => return Ok(i + 1),
            b'\\' => {
                let escaped = *bytes.get(i + 1).ok_or_else(unterminated_error)?;
                match escaped {
                    b'n' => word.push('\n'),
                    b't' => word.push('\t'),
                    b'r' => word.push('\r'),
                    b'0' => word.push('\0'),
                    b'\\' | b'"' | b'$' | b'`' => word.push(char::from(escaped)),
                    b'\n' | b'\r' => return Err(line_break_error()),
                    // Any other escape keeps the backslash (`"\d"` is `\d`), as
                    // before. Only the backslash is consumed here: the next
                    // character may be multi-byte and is copied by the next scan.
                    _ => {
                        word.push('\\');
                        i += 1;
                        continue;
                    }
                }
                i += 2;
            }
            _ => return Err(line_break_error()),
        }
    }
}

/// Resolves `\x` outside quotes (`start` is after the backslash) and returns
/// the index after it.
fn unquoted_escape(line: &str, start: usize, word: &mut String) -> Result<usize, TextForgeError> {
    let Some(c) = line[start..].chars().next() else {
        // A trailing backslash is kept, as shell_words did.
        word.push('\\');
        return Ok(start);
    };
    match c {
        'n' | 't' | 'r' | '0' => Err(parse_error(format!(
            "`\\{c}` is only an escape inside double quotes: write it as \"a\\{c}b\""
        ))),
        '\n' | '\r' => Err(line_break_error()),
        _ => {
            word.push(c);
            Ok(start + c.len_utf8())
        }
    }
}

// Errors name no instruction and carry no input: `read_from_text` fills
// `instruction` with the line, like its other parsing errors.
fn parse_error(message: String) -> TextForgeError {
    TextForgeError::new(TextForgeErrorCode::TextParsingError(message.into()), "", "")
}

fn unterminated_error() -> TextForgeError {
    parse_error("a quoted parameter is not closed: check the quotes on this line".into())
}

fn line_break_error() -> TextForgeError {
    parse_error(
        "an instruction cannot continue on the next line: end it with `;` on the same line, and write a line break as \"\\n\" inside double quotes".into(),
    )
}

/// Quotes `value` so that [`split_line`] reads it back as one word.
///
/// The output is identical to `shell_words::quote`, except for values with a
/// line break (`\n` or `\r`), which a line cannot hold raw: those are written
/// in double quotes with escapes.
///
/// # Example
///
/// ```
/// use textforge::api::TextForgeBuilderMethods;
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let path = std::env::temp_dir().join("textforge_doc_quote.textforge");
/// std::fs::write(&path, "").unwrap();
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().add_to_beginning("a\nb")?.build()?;
/// processor.write_pipeline_to_file(&id, &path)?;
/// assert_eq!(std::fs::read_to_string(&path).unwrap(), "atb \"a\\nb\";\n");
///
/// let reread = processor.get_pipeline_from_file(&path)?;
/// assert_eq!(processor.process_all(&reread, "!")?, "a\nb!");
/// # std::fs::remove_file(&path).unwrap();
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
pub fn quote(value: &str) -> Cow<'_, str> {
    if value.contains(['\n', '\r']) {
        return double_quote(value).into();
    }
    if value.is_empty() {
        return "''".into();
    }
    let mut special = false;
    let mut single_quote = false;
    for c in value.chars() {
        match c {
            '\'' => {
                single_quote = true;
                special = true;
            }
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '"' | ' ' | '\t' | '*'
            | '?' | '[' | '#' | '~' | '=' | '%' => special = true,
            _ => {}
        }
    }
    if !special {
        value.into()
    } else if !single_quote {
        format!("'{value}'").into()
    } else {
        format!("'{}'", value.replace('\'', "'\\''")).into()
    }
}

fn double_quote(value: &str) -> String {
    let mut quoted = String::with_capacity(value.len() + 2);
    quoted.push('"');
    for c in value.chars() {
        match c {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '$' => quoted.push_str("\\$"),
            '`' => quoted.push_str("\\`"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            '\0' => quoted.push_str("\\0"),
            _ => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests;
