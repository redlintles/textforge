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

/// Byte offset and byte length of the first line terminator in `text`
/// (the terminators of [`is_break`], CRLF taken whole).
///
/// Scans bytes instead of decoding characters: every terminator starts with
/// one of six bytes (`\n`, VT, FF, `\r`, and `0xC2`/`0xE2`, the lead bytes of
/// NEL and of LS/PS), and in valid UTF-8 those lead bytes only start a
/// character, so a match is checked by looking at the next one or two bytes.
/// Up to 0.3.2 the scanner decoded every character, which made `txf
/// --read-mode line` and the line instructions slower than they need to be.
fn find_break(text: &str) -> Option<(usize, usize)> {
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(found) = bytes[start..]
        .iter()
        .position(|byte| matches!(byte, b'\n' | 0x0b | 0x0c | b'\r' | 0xc2 | 0xe2))
    {
        let at = start + found;
        let next = |n: usize| bytes.get(at + n).copied();
        match bytes[at] {
            b'\r' if next(1) == Some(b'\n') => return Some((at, 2)),
            b'\n' | 0x0b | 0x0c | b'\r' => return Some((at, 1)),
            0xc2 if next(1) == Some(0x85) => return Some((at, 2)),
            0xe2 if next(1) == Some(0x80) && matches!(next(2), Some(0xa8 | 0xa9)) => {
                return Some((at, 3));
            }
            _ => start = at + 1,
        }
    }
    None
}

pub(crate) fn lines(mut remaining: &str) -> impl Iterator<Item = (&str, &str)> {
    std::iter::from_fn(move || {
        if remaining.is_empty() {
            return None;
        }
        if let Some((offset, length)) = find_break(remaining) {
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

/// Check that line `line` (base 0, Golden Rule 22) exists in a text with
/// `count` lines.
pub(crate) fn check_line(
    line: usize,
    count: usize,
    name: &'static str,
    input: &str,
) -> Result<(), TextForgeError> {
    if line >= count {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                match count {
                    0 => format!("line {line} does not exist: the text is empty"),
                    _ => format!(
                        "line {line} does not exist: the text has {} (valid lines 0 to {})",
                        crate::utils::errors::plural(count, "line"),
                        count - 1
                    ),
                }
                .into(),
            ),
            name,
            crate::utils::errors::input_excerpt(input),
        ));
    }
    Ok(())
}

/// Validate the inclusive line range `start..=end` (base 0) of a text with
/// `count` lines: `start` must exist, `start <= end`, and an `end` past the
/// last line means the last line.
pub(crate) fn check_range(
    start: usize,
    end: usize,
    count: usize,
    name: &'static str,
    input: &str,
) -> Result<(), TextForgeError> {
    check_line(start, count, name, input)?;
    // An end past the last line means the last line, as character ranges
    // clamp an end past the last character (Golden Rule 21). Before 0.3.2 it
    // was an error.
    if start > end {
        return Err(invalid(
            name,
            &format!(
                "the line range starts at {start} but ends at {end}; write it as start..=end with start <= end"
            ),
            input,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    /// The character-by-character scanner used up to 0.3.2: the reference for
    /// the byte scanner.
    fn reference(mut remaining: &str) -> Vec<(&str, &str)> {
        let mut found = Vec::new();
        while !remaining.is_empty() {
            match remaining.char_indices().find(|(_, c)| is_break(*c)) {
                Some((offset, c)) => {
                    let length = if c == '\r' && remaining[offset..].starts_with("\r\n") {
                        2
                    } else {
                        c.len_utf8()
                    };
                    found.push((&remaining[..offset], &remaining[offset..offset + length]));
                    remaining = &remaining[offset + length..];
                }
                None => {
                    found.push((remaining, ""));
                    remaining = "";
                }
            }
        }
        found
    }

    #[test]
    fn the_byte_scanner_agrees_with_the_character_scanner() {
        // Characters that share lead bytes with the terminators (U+0080..U+00BF
        // start with 0xC2, U+2000..U+203F with 0xE2), every terminator, and
        // ordinary ASCII and multibyte text.
        let alphabet = [
            "a", " ", "\n", "\r", "\r\n", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
            "\u{80}", "\u{a0}", "\u{bf}", "\u{2000}", "\u{2027}", "\u{202a}", "\u{203f}", "é",
            "日", "🍌", "e\u{301}",
        ];
        let mut state = 0x2545_f491_4f6c_dd1d_u64;
        for _ in 0..20_000 {
            let mut text = String::new();
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let length = (state % 12) as usize;
            for step in 0..length {
                let pick = (state.rotate_left(step as u32 * 5) % alphabet.len() as u64) as usize;
                text.push_str(alphabet[pick]);
            }
            assert_eq!(
                lines(&text).collect::<Vec<_>>(),
                reference(&text),
                "{text:?}"
            );
        }
    }
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
            "dlni 4 0;",
            "onln 299999 apply tua;",
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
