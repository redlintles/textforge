//! Regression tests for the usability problems found by running the published
//! 0.3.1 `txf` on everyday tasks. Each test names the behavior it fixes.
#![cfg(feature = "test_access")]

use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    text::reader::read_from_text,
    utils::errors::{TextForgeError, TextForgeErrorCode},
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
    assert_eq!(ok(&["dlni 1 8;"], text), "a\n");
    assert_eq!(ok(&["slnc 1 8;"], text), "b\nc\n");
    assert_eq!(ok(&["onlni 1 8 apply tua;"], text), "a\nB\nC\n");
    // The start must exist and reversed ranges fail.
    for line in [
        "dlni 3 8;",
        "slnc 3 4;",
        "onlni 3 8 apply tua;",
        "slnc 2 0;",
    ] {
        assert!(run(&[line], text).is_err(), "{line}");
    }
    // A single line that does not exist is still an error.
    for line in ["dln 3;", "onln 3 apply tua;", "rln 3 x;", "dlna 8;"] {
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
    assert_eq!(ok(&["onln 0 apply end;", "ate Z;"], "a\n"), "a\n");
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
    // Words in capitals are not copied as they are ("hTTPServer").
    assert_eq!(ok(&["jcmc;"], "HTTP server"), "httpServer");
    assert_eq!(ok(&["jpsc;"], "HTTP server"), "HttpServer");
    assert_eq!(ok(&["jpsc;"], "ÉLAN vital"), "ÉlanVital");
    // The same rule as jkbc and jsnc, which lowercase everything.
    assert_eq!(ok(&["jsnc;"], "HTTP server"), "http_server");
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
    let line = format!("inf 0 '{}';", piece.display());
    assert_eq!(ok(&[line.as_str()], "ab"), "+ab");
}

// --- Replacement templates --------------------------------------------------

/// Before: `rlw` and `rnw` inserted the replacement as written, so `$1` was
/// literal there while `raw`, `rfw` and `rcw` inserted the capture group.
#[test]
fn every_replace_instruction_expands_capture_groups() {
    let input = "a1 b2 c3";
    for (line, expected) in [
        ("raw '(\\w)(\\d)' '$2$1';", "1a 2b 3c"),
        ("rfw '(\\w)(\\d)' '$2$1';", "1a b2 c3"),
        ("rcw '(\\w)(\\d)' '$2$1' 2;", "1a 2b c3"),
        ("rlw '(\\w)(\\d)' '$2$1';", "a1 b2 3c"),
        ("rnw '(\\w)(\\d)' '$2$1' 1;", "a1 2b c3"),
        ("rlw '(?<l>\\w)(?<d>\\d)' '${d}${l}';", "a1 b2 3c"),
        ("rnw '(?<l>\\w)(?<d>\\d)' '${d}${l}' 0;", "1a b2 c3"),
    ] {
        assert_eq!(ok(&[line], input), expected, "{line}");
    }
    // `$$` is a literal `$` in all five.
    for line in [
        "raw b '$$5';",
        "rfw b '$$5';",
        "rcw b '$$5' 1;",
        "rlw b '$$5';",
        "rnw b '$$5' 0;",
    ] {
        assert_eq!(ok(&[line], "abc"), "a$5c", "{line}");
    }
    // Without a match the text is unchanged.
    for line in ["rlw z '$1';", "rnw z '$1' 0;", "rnw a '$1' 9;"] {
        assert_eq!(ok(&[line], "abc"), "abc", "{line}");
    }
}

// --- `inf` argument order ---------------------------------------------------

/// Before: `inf {file} {index}`, the reverse of `ins {index} {text}`.
#[test]
fn inf_takes_the_index_first_like_ins() {
    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "+").unwrap();
    let path = piece.display().to_string();
    assert_eq!(ok(&[format!("inf 1 '{path}';").as_str()], "abc"), "ab+c");
    assert_eq!(ok(&["ins 1 '+';"], "abc"), "ab+c");
    // The old order is a parsing error, not a silent misread.
    assert!(run(&[format!("inf '{path}' 1;").as_str()], "abc").is_err());
}

// --- Expression errors ------------------------------------------------------

/// Before: errors from `[[ ]]` and `eval` were labelled `api::expr` or
/// `eval.transform` and carried evalexpr's message ("The operator Add was
/// called with a wrong combination of types: [String, Int]").
#[test]
fn expression_errors_name_the_instruction_and_explain_the_problem() {
    let error = |lines: &[&str]| -> TextForgeError {
        let mut processor = TextForgeProcessor::new();
        let id =
            processor.add_transform(lines.iter().map(|l| read_from_text(l).unwrap()).collect());
        processor.process_all(&id, "x").unwrap_err()
    };
    let e = error(&["var a = 2.5;", "ate '[[a + 1]]';"]);
    assert_eq!(e.instruction, "ate '[[a + 1]]';");
    let message = format!("{:?}", e.error_code);
    assert!(
        message.contains("`+` cannot be applied to text and an integer"),
        "{message}"
    );
    assert!(!message.contains("Add was called"), "{message}");

    let e = error(&["eval 'missing + 1' in y;"]);
    assert_eq!(e.instruction, "eval 'missing + 1' in y;");
    assert!(format!("{:?}", e.error_code).contains("`missing` does not exist"));

    let e = error(&["ate '[[(1 + 2]]';"]);
    assert!(format!("{:?}", e.error_code).contains("`(` is never closed"));
    for e in [error(&["ate '[[1 / 0]]';"]), error(&["ate '[[b]]';"])] {
        assert!(!e.instruction.contains("api::expr") && !e.instruction.contains("eval.transform"));
    }
}

/// Before: `\n` inside a quoted text of an expression was evalexpr's
/// "Illegal escape sequence" error, although `"\n"` works in parameters.
#[test]
fn quoted_texts_in_expressions_accept_control_escapes() {
    assert_eq!(ok(&[r#"ate '[["a\nb"]]';"#], ""), "a\nb");
    assert_eq!(ok(&[r#"ate '[["a\tb\r\0"]]';"#], ""), "a\tb\r\0");
    assert_eq!(ok(&[r#"ate '[["q\"\\"]]';"#], ""), "q\"\\");
    assert_eq!(ok(&[r#"ate '[[len("a\nb")]]';"#], ""), "3");
    assert_eq!(
        ok(&[r#"eval '"a" + "\n"' in v;"#, "ate '{{v}}';"], ""),
        "a\n"
    );
    // Outside quotes a backslash is not an escape.
    assert!(run(&[r#"ate '[[1 \n 2]]';"#], "").is_err());
    // An unknown escape is still an error, with the valid ones listed.
    let Err(TextForgeErrorCode::InvalidExprError(message)) = run(&[r#"ate '[["a\qb"]]';"#], "")
    else {
        panic!("expected an expression error");
    };
    assert!(
        message.contains(r"`\q`") && message.contains(r"`\n`"),
        "{message}"
    );
}

// --- `val` ------------------------------------------------------------------

/// Before: a second `val` or `var` with the same name replaced a `val`, so
/// `val` only stopped `mutv` and `eval … in`.
#[test]
fn a_val_cannot_be_defined_again() {
    for second in [
        "val a = 2;",
        "var a = 2;",
        "mutv a = 2;",
        "eval '1 + 1' in a;",
    ] {
        assert!(
            matches!(
                run(&["val a = 1;", second], ""),
                Err(TextForgeErrorCode::NonMutableVariableError(_))
            ),
            "{second}"
        );
    }
    // A `var` can be defined again, and so can a name never used by `val`.
    assert_eq!(ok(&["var a = 1;", "var a = 2;", "ate {{a}};"], ""), "2");
    assert_eq!(ok(&["var a = 1;", "val a = 2;", "ate {{a}};"], ""), "2");
    // Each run starts with a fresh context, so a `val` does not leak.
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![read_from_text("val a = 1;").unwrap()]);
    for _ in 0..2 {
        assert_eq!(processor.process_all(&id, "x").unwrap(), "x");
    }
}

// --- Expression results ----------------------------------------------------

/// Before: `[[7 / 2.0]]` gave `3.5` and `[[1 < 2]]` gave `true`, although
/// TextForge values are integers and text.
#[test]
fn expressions_give_integers_or_text() {
    for line in [
        "ate '[[7 / 2.0]]';",
        "ate '[[1.5 * 2]]';",
        "ate '[[1 < 2]]';",
        "ate '[[true]]';",
        "ate '[[(1, 2)]]';",
        "eval '2.5' in v;",
        "eval '1 == 1' in v;",
    ] {
        assert!(
            matches!(
                run(&[line], ""),
                Err(TextForgeErrorCode::InvalidExprError(_))
            ),
            "{line}"
        );
    }
    let Err(TextForgeErrorCode::InvalidExprError(message)) = run(&["ate '[[1.5 * 2]]';"], "")
    else {
        panic!("expected an expression error");
    };
    assert!(message.contains("decimal number 3.0"), "{message}");
    // Comparisons and decimals may still be used inside an expression.
    assert_eq!(ok(&[r#"ate '[[if(2 > 1, "yes", "no")]]';"#], ""), "yes");
    assert_eq!(ok(&["ate '[[7 / 2]]';"], ""), "3");
    assert_eq!(ok(&["ate '[[if(1.5 > 1, 1, 0)]]';"], ""), "1");
    assert_eq!(ok(&["eval '6 * 7' in v;", "ate {{v}};"], ""), "42");
}
