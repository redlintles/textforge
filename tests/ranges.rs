//! Character and word ranges are inclusive: `start == end` selects one item,
//! as in `onck`; only `start > end` is an error.
#![cfg(feature = "test_access")]

use std::borrow::Cow;

use textforge::{
    context::execution_context::GlobalExecutionContext, text::reader::read_from_text,
    utils::errors::TextForgeErrorCode,
};

fn run(line: &str, input: &str) -> Result<String, TextForgeErrorCode> {
    read_from_text(line)
        .and_then(|token| {
            token
                .apply_token(Cow::Borrowed(input), &mut GlobalExecutionContext::new())
                .map(Cow::into_owned)
        })
        .map_err(|error| error.error_code)
}

#[test]
fn a_one_item_range_selects_that_item() {
    for (line, input, expected) in [
        ("dlc 1 1;", "aéb", "ab"),
        ("slt 1 1;", "aéb", "é"),
        ("tucc 1 1;", "aéb", "aÉb"),
        ("tlcc 1 1;", "AÉB", "AéB"),
        ("ctc 3 3;", "ab cd", "ab Cd"),
        ("ctr 1 1;", "ab cd ef", "ab Cd ef"),
        ("onck 1 1 apply tua;", "aéb", "aÉb"),
    ] {
        assert_eq!(run(line, input).as_deref(), Ok(expected), "{line}");
    }
}

#[test]
fn a_reversed_range_is_still_an_error() {
    for line in [
        "dlc 2 1;",
        "slt 2 1;",
        "tucc 2 1;",
        "tlcc 2 1;",
        "ctc 2 1;",
        "ctr 2 1;",
    ] {
        assert!(
            matches!(
                run(line, "ab cd ef gh"),
                Err(TextForgeErrorCode::InvalidIndex(_))
            ),
            "{line}"
        );
    }
}

#[test]
fn an_end_past_the_text_is_clamped_by_every_character_range() {
    for (line, input, expected) in [
        ("tucc 1 99;", "aéb", "aÉB"),
        ("tlcc 1 3;", "AÉB", "Aéb"),
        ("slt 1 99;", "aéb", "éb"),
        ("ctc 0 99;", "ab cd", "Ab Cd"),
        ("dlc 1 99;", "aéb", "a"),
    ] {
        assert_eq!(run(line, input).as_deref(), Ok(expected), "{line}");
    }
}

#[test]
fn character_ranges_on_empty_text_are_errors() {
    // `dlc` used to return "" for any indices.
    for line in ["dlc 0 3;", "slt 0 3;", "ctc 0 3;", "tucc 0 3;", "tlcc 0 3;"] {
        assert!(
            matches!(run(line, ""), Err(TextForgeErrorCode::IndexOutOfRange(_))),
            "{line}"
        );
    }
}
