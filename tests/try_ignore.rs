//! `tryi {instruction};`: an instruction that fails is skipped and leaves no
//! trace (text, variables, blocks, replacement maps), except the safety limits,
//! which are never ignored, and files already written. See
//! `docs/error-handling.md`.
#![cfg(feature = "test_access")]

use textforge::{
    api::{
        ExecutionProfile, Limits,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    text::reader::read_from_text,
    utils::errors::TextForgeErrorCode,
};

fn register(processor: &mut TextForgeProcessor, lines: &[&str]) -> String {
    processor.add_transform(
        lines
            .iter()
            .map(|line| read_from_text(line).unwrap())
            .collect(),
    )
}

fn run(lines: &[&str], input: &str) -> String {
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, lines);
    processor.process_all(&id, input).unwrap()
}

#[test]
fn an_instruction_that_succeeds_runs_as_without_tryi() {
    assert_eq!(run(&["tryi dlc 0 1;"], "abcd"), run(&["dlc 0 1;"], "abcd"));
    assert_eq!(run(&["tryi tua;"], "ação"), "AÇÃO");
}

#[test]
fn an_instruction_that_fails_is_skipped_and_the_pipeline_continues() {
    assert_eq!(run(&["tryi dlc 9 9;", "ate !;"], "a"), "a!");
    // Without tryi the same pipeline fails.
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["dlc 9 9;", "ate !;"]);
    assert!(processor.process_all(&id, "a").is_err());
}

#[test]
fn unicode_and_empty_texts_come_back_unchanged() {
    for text in ["", "🍌", "e\u{301}é", "a\r\nb\u{2028}c"] {
        assert_eq!(run(&["tryi dls 99;"], text), text);
    }
}

/// The first example of the specification: without the snapshot the text was
/// put back but `total` kept the value the failed block gave it (`a10`).
#[test]
fn a_failed_block_leaves_no_variable_changed() {
    let lines = [
        "var total = 0;",
        "blk calc assoc mutv total = 10;",
        "blk calc assoc dlc 9 9;",
        "tryi cblk calc;",
        "ate {{total}};",
    ];
    assert_eq!(run(&lines, "a"), "a0");
}

/// The second example: a loop that fails in its third iteration. Without the
/// snapshot, the value depended on the iteration that failed.
#[test]
fn a_loop_that_fails_halfway_leaves_no_variable_changed() {
    let lines = [
        "var seen = '';",
        "blk body assoc mutv seen = {{seen}}x;",
        "blk body assoc dls 2;",
        "blk loop assoc tryi iter 3 times cblk body;",
        "cblk loop;",
        "ate [{{seen}}];",
    ];
    // "abcd" → "abd" → "ab" → dls 2 fails on the third iteration.
    assert_eq!(run(&lines, "abcd"), "abcd[]");
}

#[test]
fn a_block_that_failed_inside_tryi_can_run_again() {
    let lines = [
        "blk check assoc dls 3;",
        "tryi cblk check;",
        "ate xy;",
        "cblk check;",
    ];
    // Before the snapshot a failed cblk lost its block: the second cblk
    // failed with BlockNotFound.
    assert_eq!(run(&lines, "ab"), "abx");
}

#[test]
fn the_counter_of_an_enclosing_loop_is_kept() {
    let lines = [
        "blk step assoc tryi onln 9 apply tua;",
        "blk step assoc ate [[__COUNTER__]];",
        "blk loop assoc iter 3 times cblk step;",
        "cblk loop;",
    ];
    assert_eq!(run(&lines, "n"), "n012");
}

#[test]
fn nested_tryi_each_put_back_their_own_state() {
    let lines = [
        "var v = a;",
        "blk inner assoc mutv v = b;",
        "blk inner assoc dls 9;",
        "blk outer assoc mutv v = c;",
        "blk outer assoc tryi cblk inner;",
        "blk outer assoc ate {{v}};",
        "tryi cblk outer;",
        "ate {{v}};",
    ];
    // inner fails: v is back to c and outer goes on, appending c.
    assert_eq!(run(&lines, ""), "cc");
}

#[test]
fn end_inside_tryi_still_stops_the_pipeline() {
    assert_eq!(run(&["tryi end;", "ate x;"], "a"), "a");
}

#[test]
fn a_safety_limit_is_never_ignored() {
    let mut processor = TextForgeProcessor::new().with_profile(ExecutionProfile {
        limits: Limits {
            max_generated_bytes: Some(8),
            max_iterations: Some(2),
        },
        ..ExecutionProfile::balanced()
    });
    let in_a_block = ["blk b assoc tryi iter 3 times ate x;", "cblk b;"];
    for lines in [&["tryi rpt 100;"][..], &in_a_block[..]] {
        let id = register(&mut processor, lines);
        let error = processor.process_all(&id, "abc").unwrap_err();
        assert!(
            matches!(error.error_code, TextForgeErrorCode::LimitExceededError(_)),
            "{error}"
        );
        assert!(!error.is_ignorable());
    }
}

/// `wtf` is the one effect `tryi` cannot take back. Writing the same file
/// again replaces it, so a retry after the failure works.
#[test]
fn a_file_written_before_the_failure_stays_and_can_be_written_again() {
    let directory = tempfile::tempdir().unwrap();
    let file = directory.path().join("out.txt");
    let path = file.to_string_lossy().replace('\\', "/");
    let lines = [
        format!("blk save assoc wtf '{path}';"),
        "blk save assoc dls 9;".to_owned(),
        "tryi cblk save;".to_owned(),
        "ate !;".to_owned(),
        format!("wtf '{path}';"),
    ];
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    assert_eq!(run(&lines, "ab"), "ab!");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), "ab!");
}

#[test]
fn files_read_by_a_failed_instruction_leave_the_text_as_it_was() {
    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "PIECE").unwrap();
    let path = piece.to_string_lossy().replace('\\', "/");
    let block_lines = [
        format!("blk read assoc apf '{path}';"),
        "blk read assoc dls 99;".to_owned(),
        "tryi cblk read;".to_owned(),
    ];
    let lines: Vec<&str> = block_lines.iter().map(String::as_str).collect();
    assert_eq!(run(&lines, "ab"), "ab");
    // A missing file is an ordinary error, so it is ignored.
    assert_eq!(run(&["tryi apf 'no-such-file.txt';"], "ab"), "ab");
}

#[test]
fn the_text_format_and_bytecode_keep_tryi() {
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["tryi dlc 9 9;"]);
    assert_eq!(
        processor.get_text_transform_vec(&id).unwrap(),
        vec!["tryi dlc 9 9;\n"]
    );
}

#[test]
fn every_error_code_decides_whether_it_is_ignorable() {
    // Codes 1-31; only the limit is not ignorable.
    for code in 1..=31u16 {
        let error = code_of(code);
        assert_eq!(error.get_error_code(), code);
        assert_eq!(error.ignorable(), code != 31, "{}", error.name());
    }
}

fn code_of(code: u16) -> TextForgeErrorCode {
    use TextForgeErrorCode::*;
    let message = || "".into();
    match code {
        1 => FileNotFound(message()),
        2 => TokenNotFound(message()),
        3 => TokenArrayNotFound(message()),
        4 => FileReadingError(message()),
        5 => FileWritingError(message()),
        6 => FileOpeningError(message()),
        7 => BytecodeNotFound(message()),
        8 => BlockNotFound(message()),
        9 => VariableNotFound(message()),
        10 => NonMutableVariableError(message()),
        11 => InvalidOperands(message()),
        12 => IndexOutOfRange(message()),
        13 => InvalidIndex(message()),
        14 => InvalidParameters(message()),
        15 => InvalidArgumentNumber(message()),
        16 => BytecodeParamNotRecognized(message()),
        17 => TextParsingError(message()),
        18 => BytecodeParsingError(message()),
        19 => BytecodeParamParsingError(message()),
        20 => ValidationError(message()),
        21 => ZeroDivisionError(message()),
        22 => TryIntoFailError(message()),
        23 => IncompatibleTypeError(message()),
        24 => RequiredContextError(message()),
        25 => WatcherNotFoundError(message()),
        26 => SerializationError(message()),
        27 => GenericError(message()),
        28 => NestedBlocksNotAllowedError(message()),
        29 => InvalidUtf8Error(message()),
        30 => InvalidExprError(message()),
        31 => LimitExceededError(message()),
        _ => unreachable!(),
    }
}
