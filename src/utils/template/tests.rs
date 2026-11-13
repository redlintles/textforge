use super::*;
use crate::{
    context::{
        builtins::CurrentText,
        execution_context::{GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues},
    },
    utils::expr::interpolate_with,
};

fn context() -> GlobalExecutionContext {
    let mut context = GlobalExecutionContext::new();
    for (name, value) in [
        ("a", VarValues::String("A{{b}}[[1]]".into())),
        ("x", VarValues::Usize(5)),
        ("n", VarValues::String("7".into())),
        ("_v1", VarValues::String("word".into())),
    ] {
        context
            .add_var(
                name,
                VarEntry {
                    value,
                    mutable: true,
                },
            )
            .unwrap();
    }
    context
}

/// The compiled template and the regex interpolation agree on `input`.
fn check(input: &str, context: &GlobalExecutionContext) {
    let reader = VarReader::new(context, CurrentText::Str("one\ntwo\n"));
    let expected = interpolate_with(input, &reader).map_err(|error| format!("{error:?}"));
    let reader = VarReader::new(context, CurrentText::Str("one\ntwo\n"));
    let actual = match Template::parse(input) {
        Some(template) => template
            .render(&reader)
            .map_err(|error| format!("{error:?}")),
        None => Ok(input.to_owned()),
    };
    assert_eq!(actual, expected, "input: {input:?}");
}

#[test]
fn matches_interpolation_on_known_cases() {
    let context = context();
    for input in [
        "",
        "plain",
        "{{a}}",
        "{{x}}+{{n}}",
        "{{ a }}",
        "{{1a}}",
        "{{a",
        "a}}",
        "{{{a}}}",
        "[[1 + 2 * 3]]",
        "[[x * 2]]",
        "[[n + x]]",
        "[[]]",
        "[[1 +]]",
        "[[a\nb]]",
        "[[a]]]",
        "[[[1]]]",
        "\\{{a\\}}",
        "\\{{a}}",
        "\\[[1]]",
        "\\]]",
        "\\x",
        "\\\\{{a}}",
        "{{missing}}",
        "[[missing + 1]]",
        "[[__LINES__ + 1]]",
        "{{__LEN_CHARS__}}",
        "[[max(2, 7)]]",
        "[[\"s\" + \"t\"]]",
        "é{{a}}ü[[1]]ß",
        "[[1]][[2]]{{x}}",
        "}}{{",
        "]][[",
        "{{é}}",
        "[[x = 1]]",
        "[[1 / 0]]",
        "[[true && false]]",
        "[[(1, 2)]]",
        "v={{x}};",
    ] {
        check(input, &context);
    }
}

#[test]
fn matches_interpolation_on_generated_cases() {
    let context = context();
    let pieces = [
        "{{",
        "}}",
        "[[",
        "]]",
        "\\",
        "a",
        "x",
        "n",
        "_v1",
        "1",
        " + ",
        "\n",
        "é",
        "{",
        "]",
        "__LINES__",
        "missing",
        "*",
        "(",
        ")",
        "\"",
        ",",
    ];
    let mut state: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for _ in 0..20_000 {
        let length = (next() % 9) as usize;
        let input: String = (0..length)
            .map(|_| pieces[(next() % pieces.len() as u64) as usize])
            .collect();
        check(&input, &context);
    }
}

#[test]
fn text_without_delimiters_is_not_compiled() {
    assert!(Template::parse("plain {text} [x]").is_none());
    assert!(Template::parse("a}}").is_some());
}

#[test]
fn constant_expressions_are_evaluated_once() {
    let expression = CompiledExpr::new("1 + 2 * 3");
    assert_eq!(expression.constant, Some(Ok(Value::from_int(7))));
    assert!(CompiledExpr::new("x + 1").constant.is_none());
    assert!(CompiledExpr::new("max(1, 2)").constant.is_none());
}
