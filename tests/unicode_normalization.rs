//! Public/portable behavior, construction contexts, persistence and document I/O.
use textforge::api::{
    TextForgeBlockMethods, TextForgeBuilderMethods, TextForgeConditionalMethods,
    portable::{Argument, ErrorStage, InstructionSpec, PipelinePlan, PortableProcessor},
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};

fn operation(mode: Argument) -> InstructionSpec {
    InstructionSpec::new("unorm".into(), vec![mode])
}

#[test]
fn native_portable_and_concurrent_execution_agree_for_all_modes() {
    for (mode, input, expected) in [
        ("nfc", "Cafe\u{301}", "Café"),
        ("nfd", "Café", "Cafe\u{301}"),
        ("nfkc", "Ａﬁ²é", "Afi2é"),
        ("nfkd", "Ａﬁ²é", "Afi2e\u{301}"),
    ] {
        let mut native = TextForgeProcessor::new();
        let id = native
            .create_pipeline()
            .normalize_unicode(mode)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(native.process_all(&id, input).unwrap(), expected);
        let mut plan = PipelinePlan::new();
        plan.add(operation(Argument::Text(mode.into()))).unwrap();
        let compiled = plan.compile().unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let compiled = &compiled;
                scope.spawn(move || {
                    assert_eq!(compiled.process(input).unwrap(), expected);
                });
            }
        });
    }
}

#[test]
fn variables_conditionals_blocks_and_order_with_replacement_maps() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .var("form", "nfd")
        .unwrap()
        .block_assoc("normalize", |b| {
            b.normalize_unicode("{{form}}")?;
            Ok(())
        })
        .unwrap()
        .call_block("normalize")
        .unwrap()
        .if_do_contains_each("e", |b| {
            b.normalize_unicode("nfc")?;
            Ok(())
        })
        .unwrap()
        .define_replace_map("m", "é", "E")
        .unwrap()
        .execute_replace_map("m")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "é").unwrap(), "E");
    let id = processor
        .create_pipeline()
        .var("form", "nfd")
        .unwrap()
        .normalize_unicode("{{form}}")
        .unwrap()
        .mut_var("form", "nfc")
        .unwrap()
        .normalize_unicode("{{form}}")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "é").unwrap(), "é");
}

#[test]
fn invalid_mode_errors_preserve_portable_execution_context() {
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![operation(Argument::Text("NFC".into()))])
        .unwrap();
    let error = processor.process_all(&id, "é").unwrap_err();
    assert_eq!(error.stage, ErrorStage::Execution);
    assert_eq!(error.instruction, "unorm");
    assert_eq!(error.input, "NFC");
    assert!(
        processor
            .add_transform(vec![operation(Argument::Number(1))])
            .is_err()
    );
    assert!(
        processor
            .add_transform(vec![InstructionSpec::new("unorm".into(), vec![])])
            .is_err()
    );
}

#[cfg(feature = "test_access")]
#[test]
fn text_and_bytecode_persist_all_modes_and_variable_modes() {
    use textforge::api::portable::FilePath;
    let directory = tempfile::tempdir().unwrap();
    let path =
        |name: &str| FilePath::from_text(directory.path().join(name).to_str().unwrap().into());
    let mut processor = PortableProcessor::new();
    for mode in ["nfc", "nfd", "nfkc", "nfkd"] {
        for dynamic in [false, true] {
            let mut instructions = vec![];
            if dynamic {
                instructions.push(InstructionSpec::new(
                    "var".into(),
                    vec![Argument::Text("form".into()), Argument::Text(mode.into())],
                ));
            }
            instructions.push(operation(if dynamic {
                Argument::Variable("form".into())
            } else {
                Argument::Text(mode.into())
            }));
            let id = processor.add_transform(instructions).unwrap();
            let expected = processor.process_all(&id, "Ａﬁ²é").unwrap();
            for extension in ["textforge", "textforgebc"] {
                let name = format!("pipeline.{extension}");
                std::fs::write(directory.path().join(&name), "").unwrap();
                let restored = if extension == "textforge" {
                    processor.write_pipeline_to_file(&id, &path(&name)).unwrap();
                    processor.get_pipeline_from_file(&path(&name)).unwrap()
                } else {
                    processor.write_to_bytecode_file(&id, &path(&name)).unwrap();
                    processor.read_from_bytecode_file(&path(&name)).unwrap()
                };
                assert_eq!(processor.process_all(&restored, "Ａﬁ²é").unwrap(), expected);
            }
        }
    }
}

#[cfg(feature = "test_access")]
#[test]
fn file_batches_and_spilled_buffers_preserve_cross_chunk_composition() {
    use textforge::api::portable::{BufferOptions, FilePath, FileTask, PortableBuffer};
    let directory = tempfile::tempdir().unwrap();
    let path =
        |name: &str| FilePath::from_text(directory.path().join(name).to_str().unwrap().into());
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![operation(Argument::Text("nfc".into()))])
        .unwrap();
    // The base character ends at the core's 64 KiB copying boundary.
    let input = format!("{}e\u{301}🍌", "x".repeat(65535));
    let expected = format!("{}é🍌", "x".repeat(65535));
    let options = BufferOptions {
        memory_threshold: 1,
        spill_directory: path(""),
    };
    let buffer = PortableBuffer::from_text(&input, options).unwrap();
    assert!(buffer.is_spilled());
    assert_eq!(
        processor
            .process_all_buffered(&id, buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        expected
    );
    let mut tasks = Vec::new();
    for index in 0..8 {
        let name = format!("in{index}.txt");
        std::fs::write(directory.path().join(&name), &input).unwrap();
        tasks.push(FileTask {
            input: path(&name),
            output: path(&format!("out{index}.txt")),
            pipeline_id: id.clone(),
        });
    }
    for result in processor.process_batch(tasks) {
        result.unwrap();
    }
    for index in 0..8 {
        assert_eq!(
            std::fs::read_to_string(directory.path().join(format!("out{index}.txt"))).unwrap(),
            expected
        );
    }
}
