//! Error reports (0.4.0): every error says its kind, and errors raised while a
//! pipeline runs carry the trace of instructions they left, the parameters of
//! the instruction that failed and the cause. `Display` is one plain line;
//! `report()` is the full report. See `docs/error-handling.md`.
#![cfg(feature = "test_access")]

use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    text::reader::read_from_text,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

fn fail(lines: &[&str], input: &str) -> TextForgeError {
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(
        lines
            .iter()
            .map(|line| read_from_text(line).unwrap())
            .collect(),
    );
    processor.process_all(&id, input).unwrap_err()
}

fn plain(error: &TextForgeError) -> String {
    error.report().color(false).to_string()
}

#[test]
fn display_is_one_line_without_color_even_when_color_is_forced() {
    colored::control::set_override(true);
    let error = fail(&["dlc 9 9;"], "a");
    let line = error.to_string();
    colored::control::unset_override();
    assert!(!line.contains('\u{1b}'), "{line:?}");
    assert!(!line.contains('\n'), "{line:?}");
    assert!(line.starts_with("error[12] IndexOutOfRange: "), "{line}");
    assert!(line.ends_with("(in: dlc 9 9;)"), "{line}");
}

#[test]
fn the_report_names_the_kind_the_instruction_and_its_parameters() {
    let report = plain(&fail(&["dlc 9 9;"], "a"));
    assert!(
        report.starts_with("error[12] IndexOutOfRange: "),
        "{report}"
    );
    for expected in [
        "--> dlc 9 9;",
        "→ dlc 9 9;  ← failed here",
        "start = 9",
        "end   = 9",
        "Input: a",
        "Hint: ",
    ] {
        assert!(
            report.contains(expected),
            "missing {expected:?} in\n{report}"
        );
    }
    assert!(!report.contains('\u{1b}'));
}

#[test]
fn a_report_in_color_has_the_same_text() {
    let error = fail(&["dlc 9 9;"], "a");
    let colored = error.report().color(true).to_string();
    let ansi = regex::Regex::new("\u{1b}\\[[0-9;]*m").unwrap();
    assert!(colored.contains('\u{1b}'));
    assert_eq!(ansi.replace_all(&colored, ""), plain(&error));
}

#[test]
fn parameters_show_the_values_the_instruction_ran_with() {
    let error = fail(&["var n = 7;", "dlc [[n + 2]] {{n}};"], "abc");
    let parameters = error.parameters();
    assert_eq!(parameters[0].name, "start");
    assert_eq!(parameters[0].value, "9");
    assert_eq!(parameters[0].written.as_deref(), Some("\"[[n + 2]]\""));
    assert_eq!(parameters[1].value, "7");
    let report = plain(&error);
    assert!(
        report.contains("start = 9   (written \"[[n + 2]]\")"),
        "{report}"
    );
}

#[test]
fn the_trace_goes_through_the_containers_with_what_they_were_doing() {
    let error = fail(
        &[
            "blk b assoc ate x;",
            "blk b assoc iter 3 times dls [[2 - __COUNTER__]];",
            "cblk b;",
        ],
        "a",
    );
    let trace: Vec<_> = error.trace().collect();
    assert_eq!(trace.len(), 3, "{trace:?}");
    assert!(trace[0].instruction.starts_with("cblk b"));
    assert_eq!(trace[0].note.as_deref(), Some("block `b`, step 2 of 2"));
    assert!(trace[1].instruction.starts_with("iter 3 times"));
    assert_eq!(
        trace[1].note.as_deref(),
        Some("__COUNTER__ = 0 (iteration 1 of 3)")
    );
    assert_eq!(trace[2].instruction.trim_end(), "dls 2;");
    // The parameters are those of the instruction that failed.
    assert_eq!(error.parameters()[0].name, "index");
}

#[test]
fn errors_from_a_file_carry_file_and_line() {
    let directory = tempfile::tempdir().unwrap();
    let lib = directory.path().join("lib.textforge");
    std::fs::write(&lib, "tua;\n\ndls 9;\n").unwrap();
    let main = directory.path().join("main.textforge");
    std::fs::write(&main, "ate x;\nppjn lib.textforge;\n").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&main).unwrap();
    let error = processor.process_all(&id, "a").unwrap_err();
    let locations: Vec<_> = error.trace().map(|frame| frame.location.clone()).collect();
    assert_eq!(locations.len(), 2);
    assert!(
        locations[0]
            .as_deref()
            .unwrap()
            .ends_with("main.textforge:2"),
        "{locations:?}"
    );
    assert!(
        locations[1]
            .as_deref()
            .unwrap()
            .ends_with("lib.textforge:3"),
        "{locations:?}"
    );
}

#[test]
fn file_errors_give_the_cause_apart_from_the_message() {
    let error = fail(&["apf 'no-such-file.txt';"], "a");
    assert!(!error.error_code.message().contains("os error"), "{error}");
    let cause = error.cause().unwrap();
    assert!(cause.contains("os error") || !cause.is_empty(), "{cause}");
    assert!(plain(&error).contains("Caused by: "));
}

#[test]
fn regex_errors_give_the_regex_error_as_the_cause() {
    let error = fail(&["var p = '(';", "raw {{p}} x;"], "a");
    assert_eq!(
        error.error_code.message(),
        "`(` is not a valid regular expression"
    );
    assert!(error.cause().unwrap().contains("unclosed group"));
}

#[test]
fn errors_that_never_ran_have_no_details_and_compare_as_before() {
    let Err(error) = read_from_text("dlcc 9 9;") else {
        panic!("dlcc is not an instruction")
    };
    assert!(error.details.is_none());
    assert_eq!(error.trace().count(), 0);
    let report = plain(&error);
    assert!(report.starts_with("error[2] TokenNotFound: "), "{report}");
    assert!(!report.contains("Trace"));
    // Details are not compared.
    let ran = fail(&["dlc 9 9;"], "a");
    let bare = TextForgeError::new(
        ran.error_code.clone(),
        ran.instruction.clone(),
        ran.input.clone(),
    );
    assert_eq!(ran, bare);
}

#[test]
fn every_code_has_a_name_in_its_report() {
    for code in 1..=31u16 {
        let error = TextForgeError::new(code_of(code), "", "");
        let report = plain(&error);
        let expected = format!("error[{code}] {}: ", error.error_code.name());
        assert!(report.starts_with(&expected), "{report}");
        assert!(!error.error_code.name().is_empty());
    }
}

#[test]
fn an_ignored_report_is_a_warning_that_says_what_happened() {
    let report = fail(&["dls 9;"], "ab")
        .report()
        .ignored(true)
        .color(false)
        .to_string();
    assert!(
        report.starts_with("warning[12] IndexOutOfRange (ignored by tryi): "),
        "{report}"
    );
    assert!(report.contains("The instruction was skipped"), "{report}");
}

#[test]
fn the_safety_limits_have_their_own_code() {
    let mut processor = TextForgeProcessor::new().with_profile(textforge::api::ExecutionProfile {
        limits: textforge::api::Limits {
            max_generated_bytes: Some(4),
            max_iterations: None,
        },
        ..textforge::api::ExecutionProfile::balanced()
    });
    let id = processor.add_transform(vec![read_from_text("rpt 9;").unwrap()]);
    let error = processor.process_all(&id, "ab").unwrap_err();
    // Before 0.4.0: ValidationError (20), like a structural rule.
    assert_eq!(error.error_code.get_error_code(), 31);
    assert!(plain(&error).contains("tryi does not ignore limits"));
}

fn code_of(code: u16) -> TextForgeErrorCode {
    use TextForgeErrorCode::*;
    let m = || "message".into();
    [
        FileNotFound(m()),
        TokenNotFound(m()),
        TokenArrayNotFound(m()),
        FileReadingError(m()),
        FileWritingError(m()),
        FileOpeningError(m()),
        BytecodeNotFound(m()),
        BlockNotFound(m()),
        VariableNotFound(m()),
        NonMutableVariableError(m()),
        InvalidOperands(m()),
        IndexOutOfRange(m()),
        InvalidIndex(m()),
        InvalidParameters(m()),
        InvalidArgumentNumber(m()),
        BytecodeParamNotRecognized(m()),
        TextParsingError(m()),
        BytecodeParsingError(m()),
        BytecodeParamParsingError(m()),
        ValidationError(m()),
        ZeroDivisionError(m()),
        TryIntoFailError(m()),
        IncompatibleTypeError(m()),
        RequiredContextError(m()),
        WatcherNotFoundError(m()),
        SerializationError(m()),
        GenericError(m()),
        NestedBlocksNotAllowedError(m()),
        InvalidUtf8Error(m()),
        InvalidExprError(m()),
        LimitExceededError(m()),
    ]
    .into_iter()
    .nth(usize::from(code) - 1)
    .unwrap()
}

#[cfg(feature = "cli")]
mod txf {
    use std::io::Write;
    use std::process::{Command, Output, Stdio};

    fn txf(pipeline: &str, input: &str, extra: &[&str]) -> Output {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("p.textforge");
        std::fs::write(&path, pipeline).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_txf"))
            .arg("-f")
            .arg(&path)
            .args(extra)
            .current_dir(directory.path())
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }

    #[test]
    fn txf_prints_the_report_of_a_failure() {
        let output = txf("ate x;\niter 2 times dls 5;\n", "ab", &[]);
        assert_eq!(output.status.code(), Some(1));
        let stderr = String::from_utf8_lossy(&output.stderr);
        for expected in [
            "error[12] IndexOutOfRange: ",
            "Trace (outermost first):",
            "p.textforge:2",
            "index = 5",
        ] {
            assert!(
                stderr.contains(expected),
                "missing {expected:?} in\n{stderr}"
            );
        }
        assert!(!stderr.contains('\u{1b}'), "NO_COLOR is set");
    }

    #[test]
    fn errors_ignored_by_tryi_are_silent_without_debug() {
        let output = txf("tryi dls 9;\nate !;\n", "ab", &[]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "ab!");
        assert!(
            output.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn debug_shows_each_ignored_error_and_a_summary() {
        let output = txf("tryi dls 9;\ntryi dlc 7 8;\nate !;\n", "ab", &["--debug"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("warning[12] IndexOutOfRange (ignored by tryi)"),
            "{stdout}"
        );
        assert!(stdout.contains("2 errors ignored by tryi:"), "{stdout}");
        assert!(stdout.contains("p.textforge:1: dls 9;"), "{stdout}");
        assert!(stdout.contains("p.textforge:2: dlc 7 8;"), "{stdout}");
        assert!(stdout.contains("After: ab!"), "{stdout}");
    }

    #[test]
    fn debug_says_which_variables_were_put_back() {
        let pipeline =
            "var v = 1;\nblk b assoc mutv v = 2;\nblk b assoc dls 9;\ntryi cblk b;\nate {{v}};\n";
        let output = txf(pipeline, "a", &["--debug"]);
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("state restored: variables v"), "{stdout}");
        assert!(stdout.contains("After: a1"), "{stdout}");
    }
}
