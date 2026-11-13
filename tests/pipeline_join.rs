//! `ppjn`: joined pipelines are loaded with the outer pipeline, run in a new
//! context, embed in bytecode and report errors with their origin.
#![cfg(feature = "test_access")]

use std::path::{Path, PathBuf};

use textforge::{
    api::{
        TextBuffer, TextBufferOptions, TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PortableProcessor},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    text::reader::read_from_text,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

fn write(directory: &Path, name: &str, lines: &[&str]) -> PathBuf {
    let path = directory.join(name);
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    path
}

/// Register the pipeline in `file` the way `txf` does.
fn load(processor: &mut TextForgeProcessor, file: &Path) -> Result<String, TextForgeError> {
    processor.get_pipeline_from_file(file)
}

#[test]
fn joined_pipelines_run_on_the_current_text_relative_to_their_file() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("lib")).unwrap();
    write(
        directory.path(),
        "lib/clean.textforge",
        &["tbs;", "", "// comment", "ate '!';"],
    );
    let main = write(
        directory.path(),
        "main.textforge",
        &["ppjn lib/clean.textforge;", "atb '> ';"],
    );
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    assert_eq!(
        processor.process_all(&id, "  olá 🎉  ").unwrap(),
        "> olá 🎉!"
    );
    // Text form keeps only the path.
    assert_eq!(
        processor.get_text_transform_vec(&id).unwrap()[0],
        "ppjn lib/clean.textforge;\n"
    );
}

#[test]
fn state_is_not_shared_with_the_joined_pipeline() {
    let directory = tempfile::tempdir().unwrap();
    write(directory.path(), "reads.textforge", &["ate '{{outer}}';"]);
    write(
        directory.path(),
        "defines.textforge",
        &[
            "var inner = 'i';",
            "blk b assoc ate 'B';",
            "rmapd m add a z;",
        ],
    );
    let mut processor = TextForgeProcessor::new();

    let reads = write(
        directory.path(),
        "a.textforge",
        &["var outer = 'o';", "ppjn reads.textforge;"],
    );
    let id = load(&mut processor, &reads).unwrap();
    let error = processor.process_all(&id, "x").unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::VariableNotFound(_)
    ));
    assert!(
        error.instruction.contains("reads.textforge:1"),
        "{}",
        error.instruction
    );

    for after in ["ate '{{inner}}';", "cblk b;", "rmape m;"] {
        let main = write(
            directory.path(),
            "b.textforge",
            &["ppjn defines.textforge;", after],
        );
        let id = load(&mut processor, &main).unwrap();
        assert!(
            processor.process_all(&id, "x").is_err(),
            "{after} saw inner state"
        );
    }
}

#[test]
fn the_static_context_is_shared() {
    let directory = tempfile::tempdir().unwrap();
    write(
        directory.path(),
        "lib.textforge",
        &["ate '[{{__PIPELINE_NAME__}}|{{__LINES__}}]';"],
    );
    let main = write(
        directory.path(),
        "principal.textforge",
        &["ppjn lib.textforge;"],
    );
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    assert_eq!(
        processor.process_all(&id, "a\nb").unwrap(),
        "a\nb[principal|2]"
    );
}

#[test]
fn end_stops_only_the_joined_pipeline() {
    let directory = tempfile::tempdir().unwrap();
    write(
        directory.path(),
        "lib.textforge",
        &["ate a;", "end;", "ate b;"],
    );
    let main = write(
        directory.path(),
        "main.textforge",
        &[
            "ppjn lib.textforge;",
            "ate c;",
            "iter 2 times ppjn lib.textforge;",
        ],
    );
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "xacaa");
}

#[test]
fn cycles_and_excessive_nesting_are_rejected_when_loading() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();

    let own = write(directory.path(), "own.textforge", &["ppjn own.textforge;"]);
    let error = load(&mut processor, &own).unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));

    write(directory.path(), "b.textforge", &["ppjn c.textforge;"]);
    write(directory.path(), "c.textforge", &["ppjn a.textforge;"]);
    let a = write(directory.path(), "a.textforge", &["ppjn b.textforge;"]);
    let error = load(&mut processor, &a).unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));
    let chain: Vec<_> = error.input.split(" → ").collect();
    assert_eq!(chain.len(), 4, "{}", error.input);
    assert!(chain[0].ends_with("a.textforge") && chain[3].ends_with("a.textforge"));

    // A chain deeper than MAX_JOIN_DEPTH (16) files.
    for level in 0..20 {
        write(
            directory.path(),
            &format!("deep{level}.textforge"),
            &[&format!("ppjn deep{}.textforge;", level + 1)],
        );
    }
    write(directory.path(), "deep20.textforge", &["null;"]);
    let error = load(&mut processor, &directory.path().join("deep0.textforge")).unwrap_err();
    assert!(matches!(
        error.error_code,
        TextForgeErrorCode::ValidationError(_)
    ));
    // Within the limit it loads.
    let shallow = write(
        directory.path(),
        "shallow.textforge",
        &["ppjn deep10.textforge;"],
    );
    assert!(load(&mut processor, &shallow).is_ok());
}

#[test]
fn joined_files_are_read_once_when_loading_not_while_running() {
    let directory = tempfile::tempdir().unwrap();
    let library = write(directory.path(), "lib.textforge", &["ate '.';"]);
    let main = write(
        directory.path(),
        "main.textforge",
        &["iter 1000 times ppjn lib.textforge;"],
    );
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    std::fs::remove_file(&library).unwrap();
    assert_eq!(processor.process_all(&id, "").unwrap(), ".".repeat(1000));
}

#[test]
fn errors_name_the_joined_file_and_line_without_its_contents() {
    let directory = tempfile::tempdir().unwrap();
    write(
        directory.path(),
        "lib.textforge",
        &["// header", "tbs;", "dlc 99 100;"],
    );
    let main = write(directory.path(), "main.textforge", &["ppjn lib.textforge;"]);
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    let error = processor.process_all(&id, "abc").unwrap_err();
    assert!(
        error.instruction.starts_with("ppjn lib.textforge:3 › "),
        "{}",
        error.instruction
    );

    // A parse error in the joined file is reported when loading.
    write(
        directory.path(),
        "broken.textforge",
        &["tbs;", "notaninstruction;"],
    );
    let main = write(
        directory.path(),
        "uses-broken.textforge",
        &["ppjn broken.textforge;"],
    );
    let error = load(&mut processor, &main).unwrap_err();
    assert!(
        error.instruction.contains("broken.textforge:2"),
        "{}",
        error.instruction
    );

    // A missing file is reported when loading, with its path.
    let main = write(
        directory.path(),
        "uses-missing.textforge",
        &["ppjn missing.textforge;"],
    );
    let error = load(&mut processor, &main).unwrap_err();
    assert!(error.input.contains("missing.textforge"));
}

#[test]
fn the_path_must_be_a_literal() {
    for line in [
        "ppjn '{{lib}}';",
        "ppjn 'lib-[[1 + 1]].textforge';",
        "ppjn {{lib}};",
    ] {
        let error = read_from_text(line).err().expect("templated path accepted");
        assert!(
            matches!(error.error_code, TextForgeErrorCode::ValidationError(_)),
            "{line}: {:?}",
            error.error_code
        );
    }
}

#[test]
fn joined_bytecode_is_recognized_by_its_content() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let library = processor
        .create_pipeline()
        .add_to_end("-bc")
        .unwrap()
        .build()
        .unwrap();
    // Named .textforge on purpose: the format comes from the first bytes.
    let compiled = directory.path().join("compiled.textforgebc");
    std::fs::write(&compiled, "").unwrap();
    processor
        .write_to_bytecode_file(&library, &compiled)
        .unwrap();
    std::fs::copy(&compiled, directory.path().join("disguised.textforge")).unwrap();

    let main = write(
        directory.path(),
        "main.textforge",
        &["ppjn compiled.textforgebc;", "ppjn disguised.textforge;"],
    );
    let id = load(&mut processor, &main).unwrap();
    assert_eq!(processor.process_all(&id, "x").unwrap(), "x-bc-bc");
}

#[test]
fn bytecode_embeds_joined_pipelines_and_needs_no_files_to_run() {
    let directory = tempfile::tempdir().unwrap();
    write(directory.path(), "inner.textforge", &["ate '+';"]);
    let library = write(
        directory.path(),
        "lib.textforge",
        &["tua;", "ppjn inner.textforge;", "onln 1 apply atb '#';"],
    );
    let main = write(
        directory.path(),
        "main.textforge",
        &["ppjn lib.textforge;", "iter 2 times ppjn inner.textforge;"],
    );
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    let expected = processor.process_all(&id, "ab").unwrap();
    assert_eq!(expected, "#AB+++");

    let compiled = directory.path().join("main.textforgebc");
    std::fs::write(&compiled, "").unwrap();
    processor.write_to_bytecode_file(&id, &compiled).unwrap();
    std::fs::remove_file(&library).unwrap();
    std::fs::remove_file(directory.path().join("inner.textforge")).unwrap();

    let restored = processor.read_from_bytecode_file(&compiled).unwrap();
    assert_eq!(processor.process_all(&restored, "ab").unwrap(), expected);
    // Back to text: the path, not the embedded pipeline.
    assert_eq!(
        processor.get_text_transform_vec(&restored).unwrap()[0],
        "ppjn lib.textforge;\n"
    );
    // And from that bytecode to bytecode again.
    let again = directory.path().join("again.textforgebc");
    std::fs::write(&again, "").unwrap();
    processor.write_to_bytecode_file(&restored, &again).unwrap();
    let twice = processor.read_from_bytecode_file(&again).unwrap();
    assert_eq!(processor.process_all(&twice, "ab").unwrap(), expected);
}

#[test]
fn every_processing_mode_gives_the_same_result() {
    let directory = tempfile::tempdir().unwrap();
    write(
        directory.path(),
        "lib.textforge",
        &[
            "var n = 2;",
            "blk b assoc ate '{{n}}';",
            "cblk b;",
            "rmapd m add o 0;",
            "rmape m;",
            "onln 2 apply tua;",
        ],
    );
    let main = write(
        directory.path(),
        "main.textforge",
        &["ppjn lib.textforge;", "ate ' fim';"],
    );
    let input = "um dois\ntrês 🍌\r\nquatro";
    let mut processor = TextForgeProcessor::new();
    let id = load(&mut processor, &main).unwrap();
    let plain = processor.process_all(&id, input).unwrap();
    assert_eq!(plain, "um d0is\nTRÊS 🍌\r\nquatr02 fim");
    assert_eq!(processor.process_all_with_debug(&id, input).unwrap(), plain);
    let buffer = TextBuffer::from_text(input, TextBufferOptions::new(4, directory.path())).unwrap();
    assert_eq!(
        processor
            .process_all_buffered(&id, buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        plain
    );
    let mut watchers = textforge::watchers::WatcherList::new();
    let report = directory.path().join("report.json");
    assert_eq!(
        processor
            .process_all_with_watchers(&id, input, &mut watchers, &report)
            .unwrap(),
        plain
    );
}

#[test]
fn builders_portable_plans_and_sections_join_pipelines() {
    let directory = tempfile::tempdir().unwrap();
    let library = write(
        directory.path(),
        "lib.textforge",
        &["tua;", "ate '{{__LEN_CHARS__}}';"],
    );
    let path = library.to_string_lossy().into_owned();

    let mut processor = TextForgeProcessor::new();
    let built = processor
        .create_pipeline()
        .pipeline_join(path.as_str())
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&built, "ab").unwrap(), "AB2");

    // A missing file fails the build.
    assert!(
        processor
            .create_pipeline()
            .pipeline_join("missing.textforge")
            .and_then(|builder| builder.build())
            .is_err()
    );

    let mut portable = PortableProcessor::new();
    let id = portable
        .add_transform(vec![InstructionSpec::new(
            "ppjn".into(),
            vec![Argument::Text(path.clone())],
        )])
        .unwrap();
    assert_eq!(portable.process_all(&id, "ab").unwrap(), "AB2");
    let snapshot = portable.get_transform_vec(&id).unwrap();
    assert_eq!(snapshot[0].arguments, vec![Argument::Text(path.clone())]);

    let line = format!("onln 2 apply ppjn '{path}';");
    let id = processor.add_transform(vec![read_from_text(&line).unwrap()]);
    assert_eq!(
        processor.process_all(&id, "ab\ncde\nf").unwrap(),
        "ab\nCDE3\nf"
    );
}
