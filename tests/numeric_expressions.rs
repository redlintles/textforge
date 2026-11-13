//! Variable names with underscores and `[[ ]]`/`{{ }}` expressions in numeric
//! parameters (e.g. `onln '[[__COUNTER__ + 1]]' apply …`).
#![cfg(feature = "test_access")]

use std::borrow::Cow;

use textforge::{
    api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    context::execution_context::GlobalExecutionContext,
    text::reader::read_from_text,
    utils::errors::TextForgeErrorCode,
};

fn register(processor: &mut TextForgeProcessor, lines: &[&str]) -> String {
    processor.add_transform(lines.iter().map(|l| read_from_text(l).unwrap()).collect())
}

fn run(lines: &[&str], input: &str) -> Result<String, TextForgeErrorCode> {
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, lines);
    processor.process_all(&id, input).map_err(|e| e.error_code)
}

#[test]
fn variable_names_accept_underscores() {
    assert_eq!(
        run(&["iter 3 times ate '{{__COUNTER__}}';"], "n").unwrap(),
        "n012"
    );
    assert_eq!(
        run(&["val line_no = 2;", "ate ' {{line_no}}';"], "n").unwrap(),
        "n 2"
    );
    assert_eq!(run(&["val _x = 1;", "dla {{_x}};"], "abc").unwrap(), "ab");
    // Escaped delimiters still print literally.
    assert_eq!(run(&["ate '\\{{a_b\\}}';"], "").unwrap(), "{{a_b}}");
    // Names still cannot start with a digit; such markers are dropped as before.
    assert_eq!(run(&["ate '{{1a}}';"], "").unwrap(), "1a");
}

#[test]
fn per_line_iteration_with_an_expression() {
    // Specification §6.1.7, inside a block because of the root depth limit.
    let output = run(
        &[
            "blk each assoc iter 3 times onln '[[__COUNTER__ + 1]]' apply tbs;",
            "cblk each;",
        ],
        " a \n b \n c \n d ",
    )
    .unwrap();
    assert_eq!(output, "a\nb\nc\n d ");
}

#[test]
fn numeric_parameters_accept_expressions_and_mixed_templates() {
    assert_eq!(
        run(&["val n = 2;", "dla [[n+1]];"], "abcdef").unwrap(),
        "abcd"
    );
    assert_eq!(
        run(&["val a = 1;", "dla '{{a}}{{a}}';"], &"x".repeat(15)).unwrap(),
        "x".repeat(12)
    );
    assert_eq!(run(&["dlc '[[1]]' '[[2 * 2]]';"], "abcdef").unwrap(), "af");
}

#[test]
fn expressions_that_are_not_numbers_are_errors() {
    assert!(matches!(
        run(&["dla '[[\"x\"]]';"], "abc"),
        Err(TextForgeErrorCode::IncompatibleTypeError(_))
    ));
    assert!(matches!(
        run(&["dla '[[0 - 1]]';"], "abc"),
        Err(TextForgeErrorCode::IncompatibleTypeError(_))
    ));
    assert!(matches!(
        run(&["dla '[[missing + 1]]';"], "abc"),
        Err(TextForgeErrorCode::InvalidExprError(_))
    ));
    // Plain text that is not a template is still rejected while parsing.
    assert!(read_from_text("dla abc;").is_err());
}

#[test]
fn text_and_bytecode_round_trips_keep_the_expression() {
    let directory = tempfile::tempdir().unwrap();
    let text_path = directory.path().join("expr.textforge");
    let bytecode_path = directory.path().join("expr.textforgebc");
    std::fs::File::create(&text_path).unwrap();
    std::fs::File::create(&bytecode_path).unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = register(
        &mut processor,
        &[
            "val n = 1;",
            "onln '[[n + 1]]' apply tua;",
            "onck '[[n - 1]]' '{{n}}' apply tua;",
        ],
    );
    assert_eq!(
        processor.get_text_transform_vec(&id).unwrap()[1],
        "onln '[[n + 1]]' apply tua;\n"
    );
    let input = "abc\ndef";
    let expected = processor.process_all(&id, input).unwrap();
    assert_eq!(expected, "ABc\nDEF");

    processor.write_pipeline_to_file(&id, &text_path).unwrap();
    let from_text = processor.get_pipeline_from_file(&text_path).unwrap();
    assert_eq!(processor.process_all(&from_text, input).unwrap(), expected);

    processor
        .write_to_bytecode_file(&id, &bytecode_path)
        .unwrap();
    let from_bytecode = processor.read_from_bytecode_file(&bytecode_path).unwrap();
    assert_eq!(
        processor.process_all(&from_bytecode, input).unwrap(),
        expected
    );
}

#[test]
fn builder_and_portable_accept_expression_text() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .delete_after("[[1 + 1]]")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "abcdef").unwrap(), "abc");

    let mut plan = PipelinePlan::new();
    plan.add(InstructionSpec::new(
        "dla".into(),
        vec![Argument::Text("[[1 + 1]]".into())],
    ))
    .unwrap();
    assert_eq!(plan.compile().unwrap().process("abcdef").unwrap(), "abc");

    // Text that is not a template still fails validation in a numeric slot.
    let mut plan = PipelinePlan::new();
    plan.add(InstructionSpec::new(
        "dla".into(),
        vec![Argument::Text("2".into())],
    ))
    .unwrap();
    assert!(plan.compile().is_err());
}

#[test]
fn templates_fail_when_their_variable_is_missing() {
    let token = read_from_text("dla '[[__COUNTER__]]';").unwrap();
    let mut context = GlobalExecutionContext::new();
    assert!(
        token
            .apply_token(Cow::Borrowed("abc"), &mut context)
            .is_err()
    );
}
