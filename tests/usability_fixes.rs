//! Regression tests for the usability problems found by running the published
//! 0.3.1 `txf` on everyday tasks. Each test names the behavior it fixes.
#![cfg(feature = "test_access")]

use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    text::reader::read_from_text,
    utils::errors::TextForgeErrorCode,
};

fn run(lines: &[&str], input: &str) -> Result<String, TextForgeErrorCode> {
    let mut processor = TextForgeProcessor::new();
    let mut tokens = Vec::new();
    for line in lines {
        tokens.push(read_from_text(line).map_err(|e| e.error_code)?);
    }
    let id = processor.add_transform(tokens);
    processor.process_all(&id, input).map_err(|e| e.error_code)
}

fn ok(lines: &[&str], input: &str) -> String {
    run(lines, input).unwrap()
}

// --- Interpolation delimiters -------------------------------------------

/// Before: `}}`, `]]`, `{{` and `[[` that formed no marker were deleted.
#[test]
fn stray_delimiters_stay_in_the_text() {
    assert_eq!(ok(&[r#"ate '{"a":{"b":1}}';"#], ""), r#"{"a":{"b":1}}"#);
    assert_eq!(ok(&["ate 'm[i[0]]';"], ""), "m[i[0]]");
    assert_eq!(ok(&["ate 'a}}b]]c';"], ""), "a}}b]]c");
    assert_eq!(ok(&["ate '{{';"], ""), "{{");
    assert_eq!(ok(&["ate '[[';"], ""), "[[");
    assert_eq!(ok(&["ate '{{}}[[]][[ ]]';"], ""), "{{}}[[]][[ ]]");
}

/// Before: `{{user.name}}` printed `user.name` and `{{ name }}` printed ` name `.
#[test]
fn markers_with_invalid_names_stay_as_written() {
    assert_eq!(ok(&["ate '{{user.name}}';"], ""), "{{user.name}}");
    assert_eq!(ok(&["ate 'Hello {{ name }}!';"], ""), "Hello {{ name }}!");
    assert_eq!(ok(&["ate '{{a-b}}';"], ""), "{{a-b}}");
}

/// Before: `[[:alpha:]]` was evaluated as an expression and nested classes
/// lost their `]]`.
#[test]
fn regex_classes_are_not_expressions() {
    assert_eq!(ok(&["raw '[[:alpha:]]' X;"], "a1"), "X1");
    assert_eq!(ok(&["raw '[[:alpha:][:digit:]]+' X;"], "a1-"), "X-");
    assert_eq!(ok(&["raw '[0-9[:alpha:]]' X;"], "a1-"), "XX-");
    assert_eq!(ok(&["raw '[[a-b]c]' X;"], "abc"), "XXX");
    assert_eq!(ok(&["klm '^[[:alpha:]]+$';"], "abc\n123\n"), "abc\n");
}

#[test]
fn valid_markers_and_escapes_still_work() {
    assert_eq!(ok(&["var a = 2;", "ate '{{a}}[[a + 1]]';"], ""), "23");
    assert_eq!(ok(&[r"ate '\{{a\}}\[[1\]]';"], ""), "{{a}}[[1]]");
    assert_eq!(ok(&[r"ate '\{{a}}';"], ""), "{{a}}");
    assert!(matches!(
        run(&["ate '{{missing}}';"], ""),
        Err(TextForgeErrorCode::VariableNotFound(_))
    ));
}

/// Before: an expression holding a line break was not recognized and its text
/// (quotes included) was printed.
#[test]
fn expressions_may_hold_line_breaks() {
    assert_eq!(ok(&[r#"ate "[[\"a\nb\"]]";"#], ""), "a\nb");
}

// --- Variables -------------------------------------------------------------

/// Before: `var a-b = 2;` was accepted and `{{a-b}}` printed `a-b`.
#[test]
fn variable_names_must_be_identifiers() {
    for name in ["1a", "a-b", "a.b", "ção", ""] {
        for instruction in ["var", "val"] {
            let line = format!("{instruction} '{name}' = 2;");
            assert!(
                matches!(
                    run(&[&line], ""),
                    Err(TextForgeErrorCode::ValidationError(_))
                        | Err(TextForgeErrorCode::TextParsingError(_))
                ),
                "{line}"
            );
        }
    }
    assert_eq!(ok(&["var _a1 = 2;", "ate {{_a1}};"], ""), "2");
}

/// Before: the messages named the wrong instructions (`var` for a `val`,
/// `mutv` to define a variable).
#[test]
fn variable_errors_name_the_right_instructions() {
    let message = |result: Result<String, TextForgeErrorCode>| match result {
        Err(TextForgeErrorCode::NonMutableVariableError(m))
        | Err(TextForgeErrorCode::VariableNotFound(m)) => m.to_string(),
        other => panic!("unexpected {other:?}"),
    };
    let immutable = message(run(&["val a = 1;", "mutv a = 2;"], ""));
    assert!(immutable.contains("defined with `val`"), "{immutable}");
    assert!(immutable.contains("define it with `var`"), "{immutable}");
    let missing = message(run(&["mutv zz = 2;"], ""));
    assert!(missing.contains("define it with `var` before"), "{missing}");
    let unread = message(run(&["ate {{x}};"], ""));
    assert!(unread.contains("`var` or `val`"), "{unread}");
}

/// Before: `len` counted bytes while every index counts characters.
#[test]
fn len_counts_characters() {
    assert_eq!(ok(&[r#"ate '[[len("héllo👍")]]';"#], ""), "6");
    assert_eq!(ok(&["var w = 'ação';", "ate '[[len(w)]]';"], ""), "4");
    assert_eq!(ok(&["ate '[[len((1, 2, 3))]]';"], ""), "3");
}

// --- Out-of-range rule: a single index is an error, a range end is clamped ---

/// Before: line ranges rejected an end past the last line, while character
/// and word ranges clamped it.
#[test]
fn line_ranges_clamp_an_end_past_the_last_line() {
    let text = "a\nb\nc\n";
    assert_eq!(ok(&["dlni 2 9;"], text), "a\n");
    assert_eq!(ok(&["slnc 2 9;"], text), "b\nc\n");
    assert_eq!(ok(&["onlni 2 9 apply tua;"], text), "a\nB\nC\n");
    // The start must exist, 0 is never a line, and reversed ranges fail.
    for line in [
        "dlni 4 9;",
        "slnc 4 5;",
        "onlni 4 9 apply tua;",
        "dlni 0 2;",
        "slnc 3 1;",
    ] {
        assert!(run(&[line], text).is_err(), "{line}");
    }
    // A single line that does not exist is still an error.
    for line in ["dln 4;", "onln 4 apply tua;", "rln 4 x;", "dlna 9;"] {
        assert!(
            matches!(
                run(&[line], text),
                Err(TextForgeErrorCode::IndexOutOfRange(_))
            ),
            "{line}"
        );
    }
}

// --- `end` ------------------------------------------------------------------

/// Before: `end` inside a block or a loop stopped the whole pipeline, although
/// the documentation said "the pipeline (or the current block or loop)".
#[test]
fn end_stops_the_innermost_block_or_loop() {
    assert_eq!(ok(&["ate 1;", "end;", "ate 2;"], ""), "1");
    assert_eq!(
        ok(
            &[
                "blk b assoc ate 1;",
                "blk b assoc end;",
                "blk b assoc ate 2;",
                "cblk b;",
                "ate 3;"
            ],
            ""
        ),
        "13"
    );
    assert_eq!(ok(&["iter 3 times end;", "ate Z;"], ""), "Z");
    // A block that ends early inside a loop behaves like `continue`.
    assert_eq!(
        ok(
            &[
                "blk b assoc ate x;",
                "blk b assoc end;",
                "iter 2 times cblk b;",
                "ate Z;"
            ],
            ""
        ),
        "xxZ"
    );
    // `ifdc` and `on*` are neither: `end` there stops what contains them.
    assert_eq!(ok(&["ifdc a do end;", "ate Z;"], "a"), "a");
    assert_eq!(ok(&["onln 1 apply end;", "ate Z;"], "a\n"), "a\n");
    assert_eq!(
        ok(
            &[
                "blk b assoc ifdc a do end;",
                "blk b assoc ate y;",
                "cblk b;",
                "ate Z;"
            ],
            "a"
        ),
        "aZ"
    );
}

// --- Pipeline files ---------------------------------------------------------

/// Before: a file saved with a byte order mark failed with
/// "unknown instruction `\u{feff}atb`", and `.tft` was rejected by `txf`.
#[test]
fn pipeline_files_accept_a_byte_order_mark_and_the_short_extension() {
    use textforge::utils::validations::check_file_path;

    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("p.tft");
    std::fs::write(&path, "\u{feff}atb X; // first\n# note\nate Y;\n").unwrap();

    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&path).unwrap();
    assert_eq!(processor.process_all(&id, "-").unwrap(), "X-Y");

    // Asking for a format accepts both of its extensions, and only those.
    assert!(check_file_path(&path, Some("textforge")).is_ok());
    assert!(check_file_path(&path, Some("textforgebc")).is_err());
    let bytecode = directory.path().join("p.tfb");
    std::fs::write(&bytecode, "").unwrap();
    assert!(check_file_path(&bytecode, Some("textforgebc")).is_ok());
    assert!(check_file_path(&bytecode, Some("textforge")).is_err());
}

/// Before: `jcmc` copied the first word, so the result could start uppercase.
#[test]
fn camel_case_starts_lowercase() {
    assert_eq!(ok(&["jcmc;"], "Hello World"), "helloWorld");
    assert_eq!(ok(&["jpsc;"], "hello world"), "HelloWorld");
}

// --- `ins` and `inf` at index 0 ---------------------------------------------

/// Before: index 0 inserted after the first character, so `ins` could not
/// write at the start of the text.
#[test]
fn index_zero_inserts_at_the_start_of_the_text() {
    assert_eq!(ok(&["ins 0 X;"], "banana"), "Xbanana");
    assert_eq!(ok(&["ins 0 X;"], ""), "X");
    assert_eq!(ok(&["ins 0 X;"], "éa"), "Xéa");
    // Other indices are unchanged: after the character at that position.
    assert_eq!(ok(&["ins 1 X;"], "banana"), "baXnana");
    assert_eq!(ok(&["ins 5 X;"], "banana"), "bananaX");
    assert_eq!(ok(&["ins 6 X;"], "banana"), "bananaX");
    assert!(run(&["ins 7 X;"], "banana").is_err());

    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "+").unwrap();
    let line = format!("inf '{}' 0;", piece.display());
    assert_eq!(ok(&[line.as_str()], "ab"), "+ab");
}
