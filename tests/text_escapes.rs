//! Escapes in text pipelines (issue #1): `\n`, `\t`, `\r` and `\0` inside
//! double quotes are the control characters; outside quotes they are errors;
//! single quotes and every other escape keep their previous meaning.
#![cfg(feature = "test_access")]

use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    text::reader::{read_from_file, read_from_text},
    text::writer::write_to_file,
    utils::errors::TextForgeErrorCode,
};

fn run(lines: &[&str], input: &str) -> Result<String, TextForgeErrorCode> {
    let tokens = lines
        .iter()
        .map(|line| read_from_text(line))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.error_code)?;
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(tokens);
    processor.process_all(&id, input).map_err(|e| e.error_code)
}

fn parse_error(line: &str) {
    let Err(error) = read_from_text(line) else {
        panic!("{line} should not parse");
    };
    assert!(
        matches!(error.error_code, TextForgeErrorCode::TextParsingError(_)),
        "{line}: {error:?}"
    );
}

#[test]
fn issue_reproductions_now_insert_control_characters() {
    let input = "a b\tc\nd\n";
    // Before the fix, each of these wrote a backslash and a letter.
    assert_eq!(run(&[r#"raw " " "\t";"#], input).unwrap(), "a\tb\tc\nd\n");
    assert_eq!(run(&[r#"raw "b" "\n";"#], input).unwrap(), "a \n\tc\nd\n");
    assert_eq!(run(&[r#"atb "\n";"#], input).unwrap(), "\na b\tc\nd\n");
    assert_eq!(run(&[r#"atb "\t";"#], input).unwrap(), "\ta b\tc\nd\n");
    assert_eq!(run(&[r#"ins 0 "\n";"#], input).unwrap(), "\na b\tc\nd\n");
    assert_eq!(
        run(&[r#"atb "banana\nlaranja";"#], "").unwrap(),
        "banana\nlaranja"
    );
}

#[test]
fn previous_meanings_are_kept() {
    // Single quotes are literal.
    assert_eq!(run(&[r"atb '\n';"], "x").unwrap(), "\\nx");
    // `\\n` is still a backslash and an `n`.
    assert_eq!(run(&[r#"atb "\\n";"#], "x").unwrap(), "\\nx");
    // Regex escapes still reach the regex engine.
    assert_eq!(run(&[r##"raw "\d+" "#";"##], "a12b3").unwrap(), "a#b#");
    assert_eq!(run(&[r#"raw "\n" "_";"#], "a\nb").unwrap(), "a_b");
}

#[test]
fn line_breaks_outside_double_quotes_fail_safely() {
    parse_error(r"atb banana\nlaranja;");
    parse_error(r"atb \t;");
    parse_error("atb \"banana\nlaranja\";");
    // The `;` must close the line: here it is inside the quotes.
    parse_error(r#"atb "banana\nlaranja;""#);
}

#[test]
fn emails_are_extracted_in_one_pass_per_instruction() {
    // The 0.3.0 workaround needed `iter 6 times raw …` to delete words; with
    // `\n` the pipeline splits once and filters once.
    let output = run(
        &[
            r#"raw "\s+" "\n";"#,
            r#"klm "^[A-Za-z0-9._+-]+@[A-Za-z0-9-]+[.][A-Za-z0-9.-]+$";"#,
        ],
        "contato: ana@ex.com, ou\tbeto.s@mail.org\nnada aqui",
    )
    .unwrap();
    assert_eq!(output, "beto.s@mail.org\n");
}

#[test]
fn control_characters_survive_text_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.textforge");
    // write_to_file only writes to an existing file.
    std::fs::write(&path, "").unwrap();
    let source = [r#"atb "a\nb\r\t\0\"$`\\";"#, r#"raw "x" "1\n2";"#];
    let tokens: Vec<_> = source.iter().map(|l| read_from_text(l).unwrap()).collect();
    // Before the fix, writing a parameter with a line break was an error.
    write_to_file(&path, &tokens).unwrap();
    let written = std::fs::read_to_string(&path).unwrap();
    assert_eq!(written.lines().count(), 2, "{written}");
    let reread = read_from_file(&path).unwrap();

    let run_tokens = |tokens| {
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(tokens);
        processor.process_all(&id, "x").unwrap()
    };
    assert_eq!(run_tokens(tokens), "a\nb\r\t\0\"$`\\1\n2");
    assert_eq!(run_tokens(reread), "a\nb\r\t\0\"$`\\1\n2");
}
