//! Static context and built-in variables: `__PIPELINE_NAME__`,
//! `__TEXTFORGE_VERSION__`, `__INPUT_FILE__`, `__LEN_CHARS__` and `__LINES__`.
#![cfg(feature = "test_access")]

use textforge::{
    api::{
        TextBuffer, TextBufferOptions, TextForgeBuilderMethods,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
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
fn pipeline_name_defaults_to_the_identifier_and_can_be_set()
-> Result<(), textforge::utils::errors::TextForgeError> {
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["ate '{{__PIPELINE_NAME__}}';"]);
    assert_eq!(processor.process_all(&id, "")?, id);
    assert_eq!(processor.pipeline_name(&id)?, id);

    processor.set_pipeline_name(&id, "relatório 📄")?;
    assert_eq!(processor.process_all(&id, "> ")?, "> relatório 📄");

    let missing = processor.set_pipeline_name("missing", "x").unwrap_err();
    assert!(matches!(
        missing.error_code,
        TextForgeErrorCode::TokenArrayNotFound(_)
    ));
    processor.remove_transform(&id)?;
    assert!(processor.pipeline_name(&id).is_err());
    Ok(())
}

#[test]
fn portable_processor_names_pipelines_like_the_core() {
    use textforge::api::portable::{Argument, InstructionSpec, PortableProcessor};
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![InstructionSpec::new(
            "ate".into(),
            vec![Argument::Text("={{__PIPELINE_NAME__}}".into())],
        )])
        .unwrap();
    assert_eq!(processor.pipeline_name(&id).unwrap(), id);
    processor.set_pipeline_name(&id, "portátil".into()).unwrap();
    assert_eq!(processor.pipeline_name(&id).unwrap(), "portátil");
    assert_eq!(processor.process_all(&id, "x").unwrap(), "x=portátil");
    assert!(processor.set_pipeline_name("missing", "x".into()).is_err());
}

#[test]
fn pipelines_read_from_files_are_named_after_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["ate '[{{__PIPELINE_NAME__}}]';"]);

    let text_path = directory.path().join("limpeza.textforge");
    std::fs::write(&text_path, "").unwrap();
    processor.write_pipeline_to_file(&id, &text_path).unwrap();
    let loaded = processor.get_pipeline_from_file(&text_path).unwrap();
    assert_eq!(processor.process_all(&loaded, "x").unwrap(), "x[limpeza]");

    #[cfg(feature = "bytecode")]
    {
        let bytecode_path = directory.path().join("compiled.textforgebc");
        std::fs::write(&bytecode_path, "").unwrap();
        processor
            .write_to_bytecode_file(&id, &bytecode_path)
            .unwrap();
        let loaded = processor.read_from_bytecode_file(&bytecode_path).unwrap();
        assert_eq!(processor.process_all(&loaded, "x").unwrap(), "x[compiled]");
    }
}

#[test]
fn version_is_the_core_version() {
    assert_eq!(
        run(&["ate '{{__TEXTFORGE_VERSION__}}';"], "v").unwrap(),
        format!("v{}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn input_file_is_empty_for_text_and_the_path_for_files() {
    assert_eq!(run(&["ate '[{{__INPUT_FILE__}}]';"], "x").unwrap(), "x[]");

    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["ate '|{{__INPUT_FILE__}}';"]);

    let input = directory.path().join("entrada.txt");
    let output = directory.path().join("saida.txt");
    std::fs::write(&input, "x").unwrap();
    processor.process_file(&id, &input, &output).unwrap();
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        format!("x|{}", input.display())
    );

    // Each batch task sees its own file.
    let inputs: Vec<_> = (0..4)
        .map(|i| directory.path().join(format!("in-{i}.txt")))
        .collect();
    let outputs: Vec<_> = (0..4)
        .map(|i| directory.path().join(format!("out-{i}.txt")))
        .collect();
    for path in &inputs {
        std::fs::write(path, "x").unwrap();
    }
    let tasks = inputs
        .iter()
        .zip(&outputs)
        .map(|(input, output)| (input.as_path(), id.as_str(), output.as_path()))
        .collect();
    for result in processor.process_batch(tasks) {
        result.unwrap();
    }
    for (input, output) in inputs.iter().zip(&outputs) {
        assert_eq!(
            std::fs::read_to_string(output).unwrap(),
            format!("x|{}", input.display())
        );
    }
}

#[test]
fn derived_values_follow_the_current_text() {
    // Unicode scalar values, logical lines (docs/lines.md).
    assert_eq!(
        run(
            &["ate ' {{__LEN_CHARS__}}/{{__LINES__}}';"],
            "e\u{301}👩🏽‍💻\r\nb\n"
        )
        .unwrap(),
        "e\u{301}👩🏽‍💻\r\nb\n 10/2"
    );
    assert_eq!(
        run(&["ate '{{__LEN_CHARS__}}/{{__LINES__}}';"], "").unwrap(),
        "0/0"
    );
    // Recomputed after every instruction that changes the text.
    assert_eq!(
        run(
            &["ate ' {{__LINES__}}';", "rpt 2;", "ate ' {{__LINES__}}';"],
            "a\nb"
        )
        .unwrap(),
        "a\nb 2a\nb 2 3"
    );
    assert_eq!(
        run(
            &["ate ' ({{__LINES__}})';", "atb '{{__LEN_CHARS__}}:';"],
            "primeira\nsegunda"
        )
        .unwrap(),
        "20:primeira\nsegunda (2)"
    );
}

#[test]
fn derived_values_work_in_expressions_and_numeric_parameters() {
    // The last line, whatever the document length.
    assert_eq!(
        run(&["onln '[[__LINES__]]' apply ate '!';"], "a\nb\nc").unwrap(),
        "a\nb\nc!"
    );
    assert_eq!(
        run(&["ate ' [[__LINES__ * 100 + __LEN_CHARS__]]';"], "ab\ncd").unwrap(),
        "ab\ncd 205"
    );
    // Stored through a variable reference: the value is copied when stored.
    assert_eq!(
        run(
            &[
                "var total = {{__LINES__}};",
                "ate ' more';",
                "ate '{{total}}';"
            ],
            "a\nb"
        )
        .unwrap(),
        "a\nb more2"
    );
}

#[test]
fn derived_values_describe_the_section_inside_on_instructions_and_iterations() {
    assert_eq!(
        run(&["onln 2 apply ate '={{__LEN_CHARS__}}';"], "aaaa\nbb\nc").unwrap(),
        "aaaa\nbb=2\nc"
    );
    assert_eq!(
        run(&["onlni 2 3 apply ate '={{__LINES__}}';"], "a\nb\nc\nd").unwrap(),
        "a\nb\nc=2\nd"
    );
    assert_eq!(
        run(&["onck 0 2 apply atb '{{__LEN_CHARS__}}';"], "abcdef").unwrap(),
        "3abcdef"
    );
    assert_eq!(
        run(&["iter 3 times ate '{{__LEN_CHARS__}}';"], "ab").unwrap(),
        "ab234"
    );
    // Blocks run their steps on the whole text, one after the other.
    assert_eq!(
        run(
            &[
                "blk b assoc ate 'x';",
                "blk b assoc ate '{{__LEN_CHARS__}}';",
                "cblk b;"
            ],
            "ab"
        )
        .unwrap(),
        "abx3"
    );
}

#[test]
fn built_in_names_cannot_be_defined() {
    for name in [
        "__PIPELINE_NAME__",
        "__TEXTFORGE_VERSION__",
        "__INPUT_FILE__",
        "__LEN_CHARS__",
        "__LINES__",
    ] {
        for line in [
            format!("var {name} = 1;"),
            format!("val {name} = 1;"),
            format!("mutv {name} = 1;"),
            format!("eval '1 + 1' in {name};"),
        ] {
            let error = read_from_text(&line).err().expect("built-in name accepted");
            assert!(
                matches!(error.error_code, TextForgeErrorCode::ValidationError(_)),
                "{line}: {:?}",
                error.error_code
            );
        }
    }

    let mut processor = TextForgeProcessor::new();
    // Builders validate when the pipeline is built.
    let error = processor
        .create_pipeline()
        .var("__LINES__", 3)
        .and_then(|builder| builder.build())
        .unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));
}

#[cfg(feature = "bytecode")]
#[test]
fn bytecode_cannot_define_built_in_names_either() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reserved.textforgebc");
    std::fs::write(&path, "").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["var aaLINES__ = 1;"]);
    processor.write_to_bytecode_file(&id, &path).unwrap();

    // Same length, so only the name changes in the record.
    let mut bytes = std::fs::read(&path).unwrap();
    let at = bytes
        .windows(9)
        .position(|window| window == b"aaLINES__")
        .unwrap();
    bytes[at..at + 2].copy_from_slice(b"__");
    std::fs::write(&path, bytes).unwrap();

    let error = processor.read_from_bytecode_file(&path).unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));
}

#[test]
fn names_computed_at_execution_time_are_checked_when_bound() {
    assert!(matches!(
        run(&["var n = '__LINES__';", "var '{{n}}' = 1;"], "").unwrap_err(),
        TextForgeErrorCode::ValidationError(_)
    ));
}

#[test]
fn other_double_underscore_names_stay_definable() {
    // Only the listed names are reserved (compatibility with existing pipelines).
    assert_eq!(
        run(
            &[
                "var __COUNTER__ = 7;",
                "var __mine__ = 'm';",
                "ate '{{__COUNTER__}}{{__mine__}}';"
            ],
            ""
        )
        .unwrap(),
        "7m"
    );
}

#[test]
fn every_processing_mode_gives_the_same_result() {
    let lines = [
        "ate ' {{__LINES__}}/{{__LEN_CHARS__}}';",
        "onln '[[__LINES__]]' apply atb '{{__PIPELINE_NAME__}}:';",
        "iter 2 times ate '[[__LEN_CHARS__ % 10]]';",
    ];
    let input = "um\ndois\r\ntrês 🎉";
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &lines);
    processor.set_pipeline_name(&id, "p").unwrap();

    let plain = processor.process_all(&id, input).unwrap();
    assert_eq!(plain, "um\ndois\r\np:três 🎉 3/1523");
    assert_eq!(processor.process_all_with_debug(&id, input).unwrap(), plain);

    #[cfg(feature = "buffered")]
    {
        // A tiny threshold keeps the document spooled to disk between steps.
        let options = TextBufferOptions::new(4, std::env::temp_dir());
        let buffer = TextBuffer::from_text(input, options).unwrap();
        let output = processor.process_all_buffered(&id, buffer).unwrap();
        assert_eq!(output.into_string().unwrap(), plain);
    }

    #[cfg(feature = "watchers")]
    {
        let directory = tempfile::tempdir().unwrap();
        let mut watchers = textforge::watchers::WatcherList::new();
        let report = directory.path().join("report.json");
        assert_eq!(
            processor
                .process_all_with_watchers(&id, input, &mut watchers, &report)
                .unwrap(),
            plain
        );
    }
}

#[test]
fn serialization_keeps_templates_unresolved() {
    let token = read_from_text("ate '{{__LINES__}}';").unwrap();
    assert!(
        token
            .to_text_line_unresolved()
            .unwrap()
            .contains("{{__LINES__}}")
    );
}
