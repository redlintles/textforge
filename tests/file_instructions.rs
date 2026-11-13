//! `wtf`, `apf` and `inf` through the DSL, the processor's modes, persistence
//! and batches.
#![cfg(feature = "test_access")]

use std::path::Path;

use textforge::{
    api::{
        TextBuffer, TextBufferOptions, TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PortableProcessor},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    text::reader::read_from_text,
};

fn quoted(path: &Path) -> String {
    format!("'{}'", path.display())
}

fn register(processor: &mut TextForgeProcessor, lines: &[String]) -> String {
    processor.add_transform(lines.iter().map(|l| read_from_text(l).unwrap()).collect())
}

#[test]
fn a_pipeline_can_save_intermediate_states_and_read_files() {
    let directory = tempfile::tempdir().unwrap();
    let header = directory.path().join("header.txt");
    std::fs::write(&header, "# título 📄\n").unwrap();
    let lines = vec![
        "tbs;".to_owned(),
        format!("wtf {};", quoted(&directory.path().join("trimmed.txt"))),
        "tua;".to_owned(),
        format!("apf {};", quoted(&header)),
        format!("inf {} 2;", quoted(&header)),
    ];
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &lines);
    let output = processor.process_all(&id, "  olá  ").unwrap();
    assert_eq!(output, "OLÁ# título 📄\n# título 📄\n");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("trimmed.txt")).unwrap(),
        "olá"
    );
}

#[test]
fn file_paths_are_interpolated_at_execution_time() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let lines = vec![
        format!("var dir = {};", quoted(directory.path())),
        "iter 3 times wtf '{{dir}}/{{__PIPELINE_NAME__}}-[[__COUNTER__ + 1]].txt';".to_owned(),
    ];
    let id = register(&mut processor, &lines);
    processor.set_pipeline_name(&id, "lote").unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "x");
    for n in 1..=3 {
        assert_eq!(
            std::fs::read_to_string(directory.path().join(format!("lote-{n}.txt"))).unwrap(),
            "x"
        );
    }
}

#[test]
fn file_instructions_inside_sections_see_only_the_section() {
    let directory = tempfile::tempdir().unwrap();
    let section = directory.path().join("section.txt");
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "+").unwrap();
    let lines = vec![
        format!("onln 2 apply wtf {};", quoted(&section)),
        format!("onln 1 apply apf {};", quoted(&piece)),
        format!("onck 0 1 apply inf {} 0;", quoted(&piece)),
    ];
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &lines);
    assert_eq!(
        processor.process_all(&id, "ab\ncd\n").unwrap(),
        "a+b+\ncd\n"
    );
    assert_eq!(std::fs::read_to_string(&section).unwrap(), "cd");
}

#[test]
fn text_and_bytecode_round_trips_keep_the_instructions() {
    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "é").unwrap();
    let out = directory.path().join("out dir");
    std::fs::create_dir(&out).unwrap();
    let lines = vec![
        format!("wtf '{}/{{{{__PIPELINE_NAME__}}}}.txt';", out.display()),
        format!("apf {};", quoted(&piece)),
        format!("inf {} 0;", quoted(&piece)),
    ];
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &lines);
    let expected = processor.process_all(&id, "ab").unwrap();
    assert_eq!(expected, "aébé");

    let text_path = directory.path().join("round.textforge");
    std::fs::write(&text_path, "").unwrap();
    processor.write_pipeline_to_file(&id, &text_path).unwrap();
    let from_text = processor.get_pipeline_from_file(&text_path).unwrap();
    assert_eq!(processor.process_all(&from_text, "ab").unwrap(), expected);
    assert_eq!(
        std::fs::read_to_string(out.join("round.txt")).unwrap(),
        "ab"
    );

    #[cfg(feature = "bytecode")]
    {
        let bytecode_path = directory.path().join("round_bc.textforgebc");
        std::fs::write(&bytecode_path, "").unwrap();
        processor
            .write_to_bytecode_file(&id, &bytecode_path)
            .unwrap();
        let from_bytecode = processor.read_from_bytecode_file(&bytecode_path).unwrap();
        assert_eq!(
            processor.process_all(&from_bytecode, "ab").unwrap(),
            expected
        );
        assert_eq!(
            std::fs::read_to_string(out.join("round_bc.txt")).unwrap(),
            "ab"
        );
    }
}

#[test]
fn every_processing_mode_gives_the_same_result_and_files() {
    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "[peça 🍌]").unwrap();
    let snapshot = directory.path().join("snapshot.txt");
    let lines = vec![
        format!("wtf {};", quoted(&snapshot)),
        format!("apf {};", quoted(&piece)),
        format!("inf {} 1;", quoted(&piece)),
    ];
    let input = "linha 1\r\nlinha 2";
    let expected = "li[peça 🍌]nha 1\r\nlinha 2[peça 🍌]";
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &lines);

    let check = |output: String| {
        assert_eq!(output, expected);
        assert_eq!(std::fs::read_to_string(&snapshot).unwrap(), input);
        std::fs::remove_file(&snapshot).unwrap();
    };
    check(processor.process_all(&id, input).unwrap());
    check(processor.process_all_with_debug(&id, input).unwrap());
    #[cfg(feature = "buffered")]
    {
        let buffer =
            TextBuffer::from_text(input, TextBufferOptions::new(4, directory.path())).unwrap();
        check(
            processor
                .process_all_buffered(&id, buffer)
                .unwrap()
                .into_string()
                .unwrap(),
        );
    }
    #[cfg(feature = "watchers")]
    {
        let mut watchers = textforge::watchers::WatcherList::new();
        let report = directory.path().join("report.json");
        check(
            processor
                .process_all_with_watchers(&id, input, &mut watchers, &report)
                .unwrap(),
        );
    }
}

#[test]
fn batches_write_independent_outputs() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let lines = vec![
        format!(
            "wtf '{}/copy-{{{{__LEN_CHARS__}}}}.txt';",
            directory.path().display()
        ),
        "tua;".to_owned(),
    ];
    let id = register(&mut processor, &lines);
    let inputs: Vec<_> = (1..=6)
        .map(|n| {
            let path = directory.path().join(format!("in-{n}.txt"));
            std::fs::write(&path, "x".repeat(n)).unwrap();
            path
        })
        .collect();
    let outputs: Vec<_> = (1..=6)
        .map(|n| directory.path().join(format!("out-{n}.txt")))
        .collect();
    let tasks = inputs
        .iter()
        .zip(&outputs)
        .map(|(input, output)| (input.as_path(), id.as_str(), output.as_path()))
        .collect();
    for result in processor.process_batch(tasks) {
        result.unwrap();
    }
    for n in 1..=6 {
        assert_eq!(
            std::fs::read_to_string(directory.path().join(format!("copy-{n}.txt"))).unwrap(),
            "x".repeat(n)
        );
        assert_eq!(
            std::fs::read_to_string(&outputs[n - 1]).unwrap(),
            "X".repeat(n)
        );
    }
}

#[test]
fn builders_and_portable_plans_match_the_text_form() {
    let directory = tempfile::tempdir().unwrap();
    let piece = directory.path().join("piece.txt");
    std::fs::write(&piece, "!").unwrap();
    let out = directory.path().join("out.txt");
    let piece_text = piece.to_string_lossy().into_owned();
    let out_text = out.to_string_lossy().into_owned();

    let mut processor = TextForgeProcessor::new();
    let built = processor
        .create_pipeline()
        .write_to_file(out_text.as_str())
        .unwrap()
        .append_file(piece_text.as_str())
        .unwrap()
        .insert_file(piece_text.as_str(), 0)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&built, "ab").unwrap(), "a!b!");

    let mut portable = PortableProcessor::new();
    let id = portable
        .add_transform(vec![
            InstructionSpec::new("wtf".into(), vec![Argument::Text(out_text.clone())]),
            InstructionSpec::new("apf".into(), vec![Argument::Text(piece_text.clone())]),
            InstructionSpec::new(
                "inf".into(),
                vec![Argument::Text(piece_text), Argument::Number(0)],
            ),
        ])
        .unwrap();
    assert_eq!(portable.process_all(&id, "ab").unwrap(), "a!b!");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "ab");
}
