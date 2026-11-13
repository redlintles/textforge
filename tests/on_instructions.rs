//! `onln`, `onck` and `onlni`: section semantics, equivalence with manual
//! extraction, errors, parsing, persistence and parity across processing modes.
#![cfg(feature = "test_access")]

use std::{borrow::Cow, ops::Range};

use textforge::{
    api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    context::execution_context::GlobalExecutionContext,
    text::reader::read_from_text,
    tokens::{InstructionMethods, transforms::tbs::Tbs},
};

fn run(pipeline: &str, input: &str) -> Result<String, String> {
    let token = read_from_text(pipeline).map_err(|e| format!("{e:?}"))?;
    let mut context = GlobalExecutionContext::new();
    token
        .apply_token(Cow::Borrowed(input), &mut context)
        .map(Cow::into_owned)
        .map_err(|e| format!("{:?}", e.error_code))
}

/// Independent oracle: byte range of lines `start..=end` (LF/CRLF only),
/// without the final terminator of `end`.
fn oracle_lines(input: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let mut offsets = vec![0];
    let mut contents = Vec::new();
    let mut position = 0;
    while position < input.len() {
        let rest = &input[position..];
        let (content, terminator) = match rest.find('\n') {
            Some(i) if rest[..i].ends_with('\r') => (i - 1, 2),
            Some(i) => (i, 1),
            None => (rest.len(), 0),
        };
        contents.push(position + content);
        position += content + terminator;
        offsets.push(position);
    }
    if start == 0 || start > end || end > contents.len() {
        return None;
    }
    Some(offsets[start - 1]..contents[end - 1])
}

fn oracle_chars(input: &str, start: usize, end: usize) -> Option<Range<usize>> {
    let bounds: Vec<usize> = input
        .char_indices()
        .map(|(i, _)| i)
        .chain([input.len()])
        .collect();
    let count = bounds.len() - 1;
    if start > end || start >= count {
        return None;
    }
    Some(bounds[start]..bounds[(end + 1).min(count)])
}

/// Extract the section, apply `instruction` to it alone, splice it back.
fn manual(input: &str, range: Option<Range<usize>>, instruction: &str) -> Result<String, ()> {
    let range = range.ok_or(())?;
    let section = run(&format!("{instruction};"), &input[range.clone()]).map_err(|_| ())?;
    Ok(format!(
        "{}{section}{}",
        &input[..range.start],
        &input[range.end..]
    ))
}

const INDEXED: &[&str] = &[
    "ins 1 Z",
    "dls 1",
    "dlc 0 2",
    "dla 2",
    "dlb 2",
    "dlf",
    "dll",
    "slt 1 3",
    "tucs 1",
    "tlcs 1",
    "tucc 0 2",
    "tlcc 0 2",
    "tucw 0",
    "tlcw 0",
    "cts 1",
    "ctc 0 2",
    "ctr 1 3",
    "rnw o 0 1",
    "rcw é E 1",
    "sslt ' ' 1",
    "padl x 12",
    "padr x 12",
    "dln 1",
    "dlna 1",
    "dlnb 1",
    "slnc 1 1",
    "dlni 1 1",
    "inl 1 new",
    "rln 1 new",
    "apl tail",
    "pln head",
    "revln",
    "uniqln",
    "dle",
    "tbs",
    "atb '🍌 '",
    "ate ' é'",
];

const INPUTS: &[&str] = &[
    "",
    "one two three",
    "  héllo wörld  \nsecond line e\u{301} x\r\nthird 👩🏽‍💻 line\n",
    "a\n\nb b\na\n",
    "🍌🍌 🦀\r\n é \r\n",
];

#[test]
fn every_indexed_instruction_matches_manual_extraction() {
    for input in INPUTS {
        for instruction in INDEXED {
            for line in 0..=4 {
                let expected = manual(input, oracle_lines(input, line, line), instruction);
                let actual = run(&format!("onln {line} apply {instruction};"), input).ok();
                assert_eq!(actual, expected.ok(), "onln {line} {instruction} {input:?}");
            }
            for (start, end) in [(1, 2), (2, 3), (1, 4), (2, 1), (0, 2)] {
                let expected = manual(input, oracle_lines(input, start, end), instruction);
                let actual = run(&format!("onlni {start} {end} apply {instruction};"), input).ok();
                assert_eq!(
                    actual,
                    expected.ok(),
                    "onlni {start} {end} {instruction} {input:?}"
                );
            }
            for (start, end) in [(0, 0), (1, 3), (2, 40), (4, 4), (3, 1), (60, 70)] {
                let expected = manual(input, oracle_chars(input, start, end), instruction);
                let actual = run(&format!("onck {start} {end} apply {instruction};"), input).ok();
                assert_eq!(
                    actual,
                    expected.ok(),
                    "onck {start} {end} {instruction} {input:?}"
                );
            }
        }
    }
}

#[test]
fn specification_examples() {
    assert_eq!(
        run("onln 2 apply tbs;", "  a  \n  b  \n").unwrap(),
        "  a  \nb\n"
    );
    assert_eq!(run("onck 1 3 apply dlf;", "abcde").unwrap(), "acde");
    assert_eq!(
        run("onlni 1 3 apply uniqln;", "b\nb\na\nc").unwrap(),
        "b\na\nc"
    );
    // The section excludes the final terminator of the last selected line, so a
    // filter that removes that line leaves its preceding terminator in place,
    // exactly as `uniqln` does on the extracted block by itself.
    assert_eq!(
        run("onlni 1 4 apply uniqln;", "b\na\nb\na\nc").unwrap(),
        "b\na\n\nc"
    );
}

#[test]
fn indices_inside_are_relative_to_the_section() {
    assert_eq!(
        run("onln 3 apply dlf;", "abc\ndef\nghi\n").unwrap(),
        "abc\ndef\nhi\n"
    );
    assert_eq!(run("onln 2 apply ins 0 X;", "ab\ncd").unwrap(), "ab\ncXd");
    assert_eq!(run("onck 2 4 apply tucc 0 1;", "abcdef").unwrap(), "abCDef");
}

#[test]
fn terminators_are_preserved_for_every_kind() {
    for terminator in [
        "\n", "\r\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ] {
        let input = format!(" a {terminator} b {terminator} c ");
        assert_eq!(
            run("onln 2 apply tbs;", &input).unwrap(),
            format!(" a {terminator}b{terminator} c ")
        );
        assert_eq!(
            run("onlni 1 2 apply tbs;", &input).unwrap(),
            format!("a {terminator} b{terminator} c ")
        );
    }
}

#[test]
fn unicode_sections_never_split_scalars() {
    let input = "é🍌e\u{301}👩🏽‍💻\nx";
    assert_eq!(run("onck 1 1 apply tua;", input).unwrap(), input);
    assert_eq!(
        run("onck 2 3 apply rev;", input).unwrap(),
        "é🍌\u{301}e👩🏽‍💻\nx"
    );
    assert_eq!(
        run("onln 1 apply dll;", input).unwrap(),
        "é🍌e\u{301}👩🏽\u{200d}\nx"
    );
    assert_eq!(run("onln 2 apply tua;", input).unwrap(), "é🍌e\u{301}👩🏽‍💻\nX");
}

#[test]
fn results_may_change_the_line_count() {
    assert_eq!(
        run("onlni 1 4 apply dle;", "a\n\n\nb\nc").unwrap(),
        "a\nb\nc"
    );
    assert_eq!(run("onln 2 apply dln 1;", "a\nb\nc").unwrap(), "a\n\nc");
    assert_eq!(
        run("onlni 2 3 apply joinln '+';", "a\nb\nc\nd").unwrap(),
        "a\nb+c\nd"
    );
    assert_eq!(run("onln 1 apply apl x;", "a\nb").unwrap(), "a\nx\nb");
}

#[test]
fn invalid_sections_are_errors() {
    for (pipeline, input) in [
        ("onln 0 apply tbs;", "a"),
        ("onln 2 apply tbs;", "a\n"),
        ("onln 1 apply tbs;", ""),
        ("onlni 2 1 apply tbs;", "a\nb"),
        ("onlni 0 1 apply tbs;", "a\nb"),
        ("onlni 1 3 apply tbs;", "a\nb"),
        ("onck 3 1 apply tbs;", "abcd"),
        ("onck 4 9 apply tbs;", "abcd"),
        ("onck 0 0 apply tbs;", ""),
    ] {
        assert!(run(pipeline, input).is_err(), "{pipeline} {input:?}");
    }
    assert_eq!(run("onck 2 99 apply tua;", "abcd").unwrap(), "abCD");
}

#[test]
fn inner_errors_propagate() {
    assert!(run("onln 1 apply dlc 5 9;", "ab\ncdefghijk").is_err());
}

#[test]
fn unchanged_sections_return_the_input_without_copying() {
    for pipeline in [
        "onln 2 apply tbs;",
        "onlni 1 2 apply tbs;",
        "onck 0 1 apply tbs;",
    ] {
        let token = read_from_text(pipeline).unwrap();
        let input = "ab\ncd\n";
        let mut context = GlobalExecutionContext::new();
        let output = token
            .apply_token(Cow::Borrowed(input), &mut context)
            .unwrap();
        match output {
            Cow::Borrowed(text) => assert!(std::ptr::eq(text, input)),
            Cow::Owned(_) => panic!("{pipeline} copied an unchanged input"),
        }
    }
}

#[test]
fn owned_inputs_are_edited_in_place() {
    let token = read_from_text("onln 2 apply tua;").unwrap();
    let mut context = GlobalExecutionContext::new();
    let output = token
        .apply_token(Cow::Owned("ab\ncd\nef".into()), &mut context)
        .unwrap();
    assert_eq!(output, "ab\nCD\nef");
}

#[test]
fn nested_instructions_share_the_execution_context() {
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        ["var who = mundo;", "onln 2 apply ate ' {{who}}';"]
            .iter()
            .map(|line| read_from_text(line).unwrap())
            .collect(),
    );
    assert_eq!(
        processor.process_all(&id, "olá\noi\n").unwrap(),
        "olá\noi mundo\n"
    );
}

#[test]
fn iteration_combines_with_blocks_to_reach_each_line() {
    // At the top level the parser allows one nested level; a block payload
    // allows three, so `iter … onln …` runs from a block.
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        [
            "blk each assoc iter 3 times onln 2 apply ate '[[__COUNTER__]]';",
            "cblk each;",
        ]
        .iter()
        .map(|line| read_from_text(line).unwrap())
        .collect(),
    );
    assert_eq!(processor.process_all(&id, "a\nb\nc").unwrap(), "a\nb012\nc");
    assert!(read_from_text("iter 3 times onln 1 apply tbs;").is_err());
}

#[test]
fn text_lines_round_trip() {
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        [
            "onln 2 apply tbs;",
            "onck 0 1 apply tua;",
            "onlni 1 2 apply ate 'x y';",
        ]
        .iter()
        .map(|line| read_from_text(line).unwrap())
        .collect(),
    );
    let lines = processor.get_text_transform_vec(&id).unwrap();
    assert_eq!(
        lines,
        vec![
            "onln 2 apply tbs;\n",
            "onck 0 1 apply tua;\n",
            "onlni 1 2 apply ate 'x y';\n"
        ]
    );
}

#[test]
fn bytecode_round_trip_preserves_behavior() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("on.textforgebc");
    std::fs::File::create(&path).unwrap();
    let source = [
        "onln 2 apply tbs;",
        "onck 0 1 apply tua;",
        "onlni 1 2 apply ate '!';",
        "onln {{line}} apply dlf;",
    ];
    let mut processor = TextForgeProcessor::new();
    let mut tokens = vec![read_from_text("val line = 1;").unwrap()];
    tokens.extend(source.iter().map(|line| read_from_text(line).unwrap()));
    let id = processor.add_transform(tokens);
    processor.write_to_bytecode_file(&id, &path).unwrap();
    let loaded = processor.read_from_bytecode_file(&path).unwrap();
    let input = " ab \n cd \nef";
    let expected = processor.process_all(&id, input).unwrap();
    assert_eq!(expected, "Ab \ncd!\nef");
    assert_eq!(processor.process_all(&loaded, input).unwrap(), expected);
}

#[test]
fn opcodes_are_registered() {
    for (pipeline, opcode) in [
        ("onln 1 apply tbs;", 0x52_u32),
        ("onck 0 1 apply tbs;", 0x53),
        ("onlni 1 1 apply tbs;", 0x54),
    ] {
        let token = read_from_text(pipeline).unwrap();
        assert_eq!(token.get_opcode(), opcode);
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &opcode.to_be_bytes());
    }
}

#[test]
fn all_processing_modes_agree() {
    let source = [
        "onln 2 apply tbs;",
        "onlni 1 2 apply uniqln;",
        "onck 0 0 apply tua;",
    ];
    let input = "a\n  b  \nb\nc\n".repeat(3);
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(source.iter().map(|l| read_from_text(l).unwrap()).collect());
    let expected = processor
        .run_transform(&id, Cow::Borrowed(&input))
        .unwrap()
        .into_owned();
    assert_eq!(processor.process_all(&id, &input).unwrap(), expected);
    assert_eq!(
        processor.process_all_with_debug(&id, &input).unwrap(),
        expected
    );
    let report = tempfile::NamedTempFile::new().unwrap();
    let mut watchers = textforge::watchers::WatcherList::default();
    assert_eq!(
        processor
            .process_all_with_watchers(&id, &input, &mut watchers, report.path())
            .unwrap(),
        expected
    );
    let buffered = processor
        .process_all_buffered(
            &id,
            textforge::api::TextBuffer::from_text(
                &input,
                textforge::api::TextBufferOptions::new(1, std::env::temp_dir()),
            )
            .unwrap(),
        )
        .unwrap()
        .into_string()
        .unwrap();
    assert_eq!(buffered, expected);
}

#[test]
fn builder_methods_work_inside_blocks() {
    use textforge::api::{TextForgeBlockMethods, TextForgeConditionalMethods};
    let trim = || Box::new(Tbs::default()) as Box<dyn InstructionMethods>;
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .block_assoc("b", |block| {
            block.on_line(1, trim())?;
            block.on_line_interval(2, 2, trim())?;
            block.on_chunk(0, 0, trim())?;
            Ok(())
        })
        .unwrap()
        .call_block("b")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, " x \n y ").unwrap(), "x\ny");

    // A conditional body is one nesting level below the root, and the root
    // allows only one level, like `iter` inside a conditional.
    let error = processor
        .create_pipeline()
        .if_do_contains_each("y", |body| {
            body.on_line(1, trim())?;
            Ok(())
        })
        .unwrap()
        .build()
        .unwrap_err();
    assert!(format!("{error:?}").contains("Nested token depth exceeded"));
}

#[test]
fn portable_plans_resolve_on_instructions() {
    let trim = || Argument::Instruction(Box::new(InstructionSpec::new("tbs".into(), vec![])));
    let mut plan = PipelinePlan::new();
    plan.add(InstructionSpec::new(
        "onln".into(),
        vec![Argument::Number(1), trim()],
    ))
    .unwrap();
    plan.add(InstructionSpec::new(
        "onlni".into(),
        vec![Argument::Number(2), Argument::Number(2), trim()],
    ))
    .unwrap();
    plan.add(InstructionSpec::new(
        "onck".into(),
        vec![Argument::Number(0), Argument::Number(0), trim()],
    ))
    .unwrap();
    assert_eq!(plan.compile().unwrap().process(" x \n y ").unwrap(), "x\ny");
}

#[test]
fn replacement_maps_cannot_be_defined_inside_on_instructions() {
    // on* inherits the structural rules of iter.
    let mut plan = PipelinePlan::new();
    let define = InstructionSpec::new(
        "rmapd".into(),
        vec![
            Argument::Text("m".into()),
            Argument::Text("a".into()),
            Argument::Text("b".into()),
        ],
    );
    plan.add(InstructionSpec::new(
        "onln".into(),
        vec![Argument::Number(1), Argument::Instruction(Box::new(define))],
    ))
    .unwrap();
    assert!(plan.compile().is_err());
}

#[test]
fn end_stops_the_rest_of_blocks_and_iterations() {
    // Before the fix the block kept running after `end` ("xy") and the
    // remaining iterations and block instructions ran ("y").
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        [
            "blk b assoc ate x;",
            "blk b assoc end;",
            "blk b assoc ate y;",
            "cblk b;",
            "ate z;",
        ]
        .iter()
        .map(|line| read_from_text(line).unwrap())
        .collect(),
    );
    assert_eq!(processor.process_all(&id, "").unwrap(), "x");
    let id = processor.add_transform(
        [
            "blk b assoc iter 3 times ate x;",
            "blk b assoc iter 3 times end;",
            "blk b assoc ate y;",
            "cblk b;",
        ]
        .iter()
        .map(|line| read_from_text(line).unwrap())
        .collect(),
    );
    assert_eq!(processor.process_all(&id, "").unwrap(), "xxx");
}

fn bytecode_load(token: textforge::parser::resolve_var::TokenWrapper) -> Result<String, String> {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("depth.textforgebc");
    std::fs::File::create(&path).unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![token]);
    processor.write_to_bytecode_file(&id, &path).unwrap();
    processor
        .read_from_bytecode_file(&path)
        .map_err(|error| format!("{:?}", error.error_code))
}

fn nested(
    outer: Box<dyn InstructionMethods>,
    head: Vec<textforge::parser::resolve_var::ValType>,
    child: &str,
) -> textforge::parser::resolve_var::TokenWrapper {
    use textforge::parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
    };
    let mut params = head;
    params.push(ValType::Literal(TextForgeParamTypes::Token(
        read_from_text(child).unwrap(),
    )));
    TokenWrapper::new(outer, Some(params))
}

#[test]
fn bytecode_depth_limits_match_the_text_parser() {
    use textforge::parser::resolve_var::ValType;
    use textforge::tokens::instructions::{blk::Blk, iter::Iter};
    // Three levels inside a block are valid text; bytecode used to reject them.
    let block =
        read_from_text("blk b assoc iter 2 times onln '[[__COUNTER__ + 1]]' apply tbs;").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("block.textforgebc");
    std::fs::File::create(&path).unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![block, read_from_text("cblk b;").unwrap()]);
    processor.write_to_bytecode_file(&id, &path).unwrap();
    let loaded = processor.read_from_bytecode_file(&path).unwrap();
    assert_eq!(
        processor.process_all(&loaded, " a \n b \n c ").unwrap(),
        "a\nb\n c "
    );

    // Two levels at the root are rejected by the text parser; bytecode used to accept them.
    let root = nested(
        Box::new(Iter::default()),
        vec![ValType::Literal(2.into())],
        "onln 1 apply tbs;",
    );
    assert!(
        bytecode_load(root)
            .unwrap_err()
            .contains("Nested token depth exceeded")
    );

    // A block payload cannot be another block, in bytecode as in text.
    let inner_block = nested(
        Box::new(Blk::default()),
        vec![ValType::Literal(
            textforge::parser::params::TextForgeParamTypes::String("b".into()),
        )],
        "blk c assoc tbs;",
    );
    assert!(
        bytecode_load(inner_block)
            .unwrap_err()
            .contains("A block cannot contain another block")
    );
}
