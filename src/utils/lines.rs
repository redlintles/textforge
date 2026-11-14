//! Shared logical-line scanning. Empty text has zero lines; a final terminator
//! belongs to the preceding line. CRLF is atomic. Unicode line separators are
//! recognized without splitting UTF-8 code points.
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

pub(crate) fn is_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

pub(crate) fn lines(mut remaining: &str) -> impl Iterator<Item = (&str, &str)> {
    std::iter::from_fn(move || {
        if remaining.is_empty() {
            return None;
        }
        if let Some((offset, c)) = remaining.char_indices().find(|(_, c)| is_break(*c)) {
            let length = if c == '\r' && remaining[offset..].starts_with("\r\n") {
                2
            } else {
                c.len_utf8()
            };
            let result = (&remaining[..offset], &remaining[offset..offset + length]);
            remaining = &remaining[offset + length..];
            Some(result)
        } else {
            let result = (remaining, "");
            remaining = "";
            Some(result)
        }
    })
}

pub(crate) fn invalid(name: &'static str, message: &str, input: &str) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::InvalidParameters(message.to_owned().into()),
        name,
        crate::utils::errors::input_excerpt(input),
    )
}

pub(crate) fn check_line(
    line: usize,
    count: usize,
    name: &'static str,
    input: &str,
) -> Result<(), TextForgeError> {
    if line == 0 || line > count {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!("Line {line} does not exist; expected 1..={count}").into(),
            ),
            name,
            crate::utils::errors::input_excerpt(input),
        ));
    }
    Ok(())
}

pub(crate) fn check_range(
    start: usize,
    end: usize,
    count: usize,
    name: &'static str,
    input: &str,
) -> Result<(), TextForgeError> {
    check_line(start, count, name, input)?;
    check_line(end, count, name, input)?;
    if start > end {
        return Err(invalid(name, "Line range must be ascending", input));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn logical_boundaries_and_unicode() {
        assert_eq!(lines("").count(), 0);
        assert_eq!(lines("a\n").collect::<Vec<_>>(), vec![("a", "\n")]);
        assert_eq!(
            lines("\r\n\r\n").collect::<Vec<_>>(),
            vec![("", "\r\n"), ("", "\r\n")]
        );
        assert_eq!(
            lines("é🍌e\u{301}\r\nx\ry\u{2028}z").collect::<Vec<_>>(),
            vec![
                ("é🍌e\u{301}", "\r\n"),
                ("x", "\r"),
                ("y", "\u{2028}"),
                ("z", "")
            ]
        );
    }
}

#[cfg(test)]
mod excerpt_test {
    use crate::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

    /// Index errors keep the beginning of a large document, not all of it.
    #[test]
    fn index_errors_on_large_documents_keep_an_excerpt() {
        let document = "line\n".repeat(200_000);
        for step in [
            "dlni 5 1;",
            "onln 300000 apply tua;",
            "dls 2000000;",
            "ins 2000000 x;",
        ] {
            let mut processor = TextForgeProcessor::new();
            let token = crate::text::reader::read_from_text(step).unwrap();
            let id = processor.add_transform(vec![token]);
            let error = processor.process_all(&id, &document).unwrap_err();
            assert!(
                error.input.len() < 200,
                "{step}: {} bytes",
                error.input.len()
            );
            assert!(
                error.input.ends_with("… [1000000 bytes]"),
                "{step}: {}",
                error.input
            );
            assert!(
                error.to_string().len() < 1000,
                "{step}: {}",
                &error.to_string()[..300]
            );
        }
    }
}
