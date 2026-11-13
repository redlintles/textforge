//! Section selection and replacement shared by the `on*` instructions.
//!
//! A section is a byte range of the current document. The nested instruction
//! receives only that slice, so every index it uses is relative to the section.
//! Its result replaces the slice; the rest of the document is copied once.
use std::{borrow::Cow, ops::Range};

use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::resolve_var::TokenWrapper,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        lines::{check_range, lines},
    },
};

/// Byte range covering lines `start..=end` (base 1) without the terminator of
/// the last selected line. Terminators between selected lines are included.
///
/// Scans only up to the last selected line and allocates nothing. Errors match
/// `dlni`/`slnc` exactly: zero, missing or reversed lines, and empty documents.
pub(crate) fn line_section(
    input: &str,
    start: usize,
    end: usize,
    name: &'static str,
) -> Result<Range<usize>, TextForgeError> {
    if start != 0 && start <= end {
        let mut offset = 0;
        let mut section_start = 0;
        for (number, (content, terminator)) in (1..).zip(lines(input)) {
            if number == start {
                section_start = offset;
            }
            if number == end {
                return Ok(section_start..offset + content.len());
            }
            offset += content.len() + terminator.len();
        }
    }
    // Every failure path ends here; counting lines is only needed for the message.
    check_range(start, end, lines(input).count(), name, input)?;
    Err(internal(name))
}

/// Byte range covering characters `start..=end` (base 0, Unicode scalars).
///
/// Follows `dlc`: an `end` past the last character is clamped to it, and a
/// missing `start` is an error. Unlike `dlc`, `start == end` selects one
/// character, and an empty document is an error because it has no section.
/// One `char_indices` scan, O(n) in the position of `end`.
pub(crate) fn char_section(
    input: &str,
    start: usize,
    end: usize,
    name: &'static str,
) -> Result<Range<usize>, TextForgeError> {
    if start > end {
        return Err(TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(
                "Start index must not be greater than end index".into(),
            ),
            name,
            format!("Start Index: {start}, End Index: {end}"),
        ));
    }
    let mut characters = input.char_indices();
    let Some((section_start, _)) = characters.nth(start) else {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "Start index {start} does not exist; the text has {} characters",
                    input.chars().count()
                )
                .into(),
            ),
            name,
            input.to_owned(),
        ));
    };
    let section_end = characters
        .nth(end - start)
        .map_or(input.len(), |(offset, _)| offset);
    Ok(section_start..section_end)
}

/// Apply `inner` to `input[range]` and splice its result back.
///
/// The nested instruction receives a borrowed slice. When its result equals the
/// section, `input` is returned untouched (no copy). Otherwise a borrowed input
/// is assembled once into a new buffer and an owned input is edited in place.
/// The result may contain any number of lines, including none.
pub(crate) fn apply_to_section<'a>(
    input: Cow<'a, str>,
    range: Range<usize>,
    inner: &TokenWrapper,
    context: &mut GlobalExecutionContext,
) -> Result<Cow<'a, str>, TextForgeError> {
    match input {
        Cow::Borrowed(text) => {
            let section = &text[range.clone()];
            let result = inner.apply_nested(Cow::Borrowed(section), context)?;
            if result.as_ref() == section {
                return Ok(Cow::Borrowed(text));
            }
            let (prefix, suffix) = (&text[..range.start], &text[range.end..]);
            let output = match result {
                // Grow the nested instruction's own buffer instead of copying
                // it into a third one: for large sections this halves the work
                // and the peak memory (docs/benchmarks/on-instructions).
                Cow::Owned(mut output) => {
                    output.reserve_exact(prefix.len() + suffix.len());
                    output.insert_str(0, prefix);
                    output.push_str(suffix);
                    output
                }
                Cow::Borrowed(result) => {
                    let mut output =
                        String::with_capacity(prefix.len() + result.len() + suffix.len());
                    output.push_str(prefix);
                    output.push_str(result);
                    output.push_str(suffix);
                    output
                }
            };
            Ok(Cow::Owned(output))
        }
        Cow::Owned(mut text) => {
            let replacement = {
                let section = &text[range.clone()];
                let result = inner.apply_nested(Cow::Borrowed(section), context)?;
                (result.as_ref() != section).then(|| result.into_owned())
            };
            if let Some(replacement) = replacement {
                text.replace_range(range, &replacement);
            }
            Ok(Cow::Owned(text))
        }
    }
}

fn internal(name: &'static str) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::InvalidParameters("Line section could not be located".into()),
        name,
        "",
    )
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn line_sections_exclude_only_the_final_terminator() {
        let text = "a\r\nb\nc\u{2028}d";
        assert_eq!(&text[line_section(text, 1, 1, "t").unwrap()], "a");
        assert_eq!(&text[line_section(text, 2, 3, "t").unwrap()], "b\nc");
        assert_eq!(&text[line_section(text, 4, 4, "t").unwrap()], "d");
        assert_eq!(&text[line_section(text, 1, 4, "t").unwrap()], text);
        assert_eq!(&"\n\n"[line_section("\n\n", 2, 2, "t").unwrap()], "");
    }

    #[test]
    fn line_section_errors_match_interval_instructions() {
        for (text, start, end) in [("", 1, 1), ("a", 0, 1), ("a", 1, 2), ("a\nb", 2, 1)] {
            let expected = check_range(start, end, lines(text).count(), "t", text).unwrap_err();
            assert_eq!(line_section(text, start, end, "t").unwrap_err(), expected);
        }
    }

    #[test]
    fn char_sections_use_unicode_scalars_and_clamp_the_end() {
        let text = "é🍌e\u{301}x";
        assert_eq!(&text[char_section(text, 1, 1, "t").unwrap()], "🍌");
        assert_eq!(&text[char_section(text, 2, 3, "t").unwrap()], "e\u{301}");
        assert_eq!(&text[char_section(text, 3, 99, "t").unwrap()], "\u{301}x");
        assert!(char_section(text, 5, 9, "t").is_err());
        assert!(char_section(text, 2, 1, "t").is_err());
        assert!(char_section("", 0, 0, "t").is_err());
    }
}
