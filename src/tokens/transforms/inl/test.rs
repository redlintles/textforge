use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Inl::new(1, "x").unwrap();
    for (input, expected) in [
        ("a\nb", "a\nx\nb"),
        ("é🍌é", "é🍌é\nx"),
        ("a\r\nb\r\n", "a\r\nx\r\nb\r\n"),
        ("\n", "\nx\n"),
    ] {
        assert_eq!(
            token.transform(input.into(), None).unwrap(),
            expected,
            "input={input:?}"
        );
    }
}
#[test]
fn invalid_binding_is_atomic() {
    let mut token = Inl::new(1, "x").unwrap();
    let before = token.to_textforge_line();
    assert!(
        token
            .set_params(&[String::new().into(), 1usize.into()])
            .is_err()
    );
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Inl::new(1, "x").unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn indices_are_checked_without_overflow() {
    // Line 0 is the first line (Golden Rule 22); it was rejected while
    // lines counted from 1.
    assert!(Inl::new(0, "x").is_ok());
    assert!(
        Inl::new(usize::MAX, "x")
            .unwrap()
            .transform("a".into(), None)
            .is_err()
    );
}
#[test]
fn rejects_multiline_payload() {
    assert!(Inl::new(0, "x\ny").is_err());
}
#[test]
fn empty_payload_represents_a_real_line() {
    assert_eq!(
        Inl::new(0, "").unwrap().transform("".into(), None).unwrap(),
        "\n"
    );
}
#[test]
fn the_error_counts_the_lines_of_the_text() {
    // Before, the message counted one line too many ("the text has 4 lines"
    // for a 3-line text) because the append position was counted as a line.
    let error = Inl::new(4, "x")
        .unwrap()
        .transform("a\nb\nc\n".into(), None)
        .unwrap_err();
    assert!(
        format!("{error:?}").contains("the text has 3 lines (insert before 0 to 3; 3 appends)"),
        "{error:?}"
    );
    assert_eq!(
        Inl::new(3, "x")
            .unwrap()
            .transform("a\nb\nc\n".into(), None)
            .unwrap(),
        "a\nb\nc\nx\n"
    );
}
