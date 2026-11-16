use super::*;
#[test]
fn exact_results_for_empty_unicode_and_line_endings() {
    let token = Dlna::new(0).unwrap();
    for (input, expected) in [
        ("a\nb", "a\n"),
        ("é🍌é\r\nb\r\n", "é🍌é\r\n"),
        ("\n\n", "\n"),
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
    let mut token = Dlna::new(0).unwrap();
    let before = token.to_textforge_line();
    assert!(token.set_params(&[String::new().into()]).is_err());
    assert_eq!(token.to_textforge_line(), before);
}
#[test]
fn rejects_wrong_arity() {
    let mut token = Dlna::new(0).unwrap();
    assert!(token.set_params(&[]).is_err());
}
#[test]
fn indices_are_checked_without_overflow() {
    // Line 0 is the first line (Golden Rule 22); it was rejected while
    // lines counted from 1.
    assert!(Dlna::new(0).is_ok());
    assert!(
        Dlna::new(usize::MAX)
            .unwrap()
            .transform("a".into(), None)
            .is_err()
    );
}
#[test]
fn empty_document_has_no_addressable_lines() {
    assert!(Dlna::new(0).unwrap().transform("".into(), None).is_err());
}
