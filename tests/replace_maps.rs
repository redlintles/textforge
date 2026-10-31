use textforge::api::{
    TextForgeBlockMethods, TextForgeBuilderMethods, TextForgeConditionalMethods,
    portable::{Argument, InstructionSpec, PortableProcessor},
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};

fn run(rules: &[(&str, &str)], input: &str) -> String {
    let mut processor = TextForgeProcessor::new();
    let mut builder = processor.create_pipeline();
    for (pattern, replacement) in rules {
        builder
            .define_replace_map("m", *pattern, *replacement)
            .unwrap();
    }
    let id = builder.execute_replace_map("m").unwrap().build().unwrap();
    processor.process_all(&id, input).unwrap()
}
#[test]
fn simultaneous_unicode_captures_and_overlap() {
    assert_eq!(
        run(&[("cat", "wolf"), ("wolf", "dog")], "cat wolf"),
        "wolf dog"
    );
    assert_eq!(run(&[("a", "X"), ("aa", "Y")], "aaa"), "XXX");
    assert_eq!(run(&[("^a", "X"), ("aa", "Y")], "aaa"), "XY");
    assert_eq!(run(&[("ab", "X"), ("b", "Y")], "ab"), "X");
    assert_eq!(
        run(&[("(?P<fruit>🍌)(é)", "${fruit}:$2:$$")], "🍌é"),
        "🍌:é:$"
    );
    assert_eq!(run(&[("e\u{301}", "é")], "e\u{301}🍌"), "é🍌");
    assert_eq!(run(&[("x", "y")], ""), "");
    assert_eq!(run(&[("", "_")], "🍌é"), "_🍌_é_");
    assert_eq!(run(&[("a*", "X")], "ab"), "XbX");
    assert_eq!(run(&[("x", "")], "xxx"), "");
    assert_eq!(run(&[("x", "y")], "unchanged"), "unchanged");
    assert_eq!(run(&[("cat", "first"), ("cat", "second")], "cat"), "second");
}
#[test]
fn context_isolation_conditionals_and_block_execution() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .if_do_contains_each("cat", |b| {
            b.define_replace_map("m", "cat", "wolf")?;
            Ok(())
        })
        .unwrap()
        .block_assoc("apply", |b| {
            b.execute_replace_map("m")?;
            Ok(())
        })
        .unwrap()
        .call_block("apply")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "cat").unwrap(), "wolf");
    assert!(processor.process_all(&id, "dog").is_err());
    assert_eq!(processor.process_all(&id, "cat").unwrap(), "wolf");
    assert!(
        processor
            .create_pipeline()
            .block_assoc("bad", |b| {
                b.if_do_contains_each("x", |c| {
                    c.define_replace_map("m", "a", "b")?;
                    Ok(())
                })?;
                Ok(())
            })
            .and_then(|b| b.build())
            .is_err()
    );
}
#[test]
fn interpolation_portable_and_concurrent_reuse() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .var("pattern", "cat")
        .unwrap()
        .define_replace_map("m", "{{pattern}}", "wolf")
        .unwrap()
        .execute_replace_map("m")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "cat").unwrap(), "wolf");
    let mut portable = PortableProcessor::new();
    let id = portable
        .add_transform(vec![
            InstructionSpec::new(
                "rmapd".into(),
                vec![
                    Argument::Text("m".into()),
                    Argument::Text("cat".into()),
                    Argument::Text("wolf".into()),
                ],
            ),
            InstructionSpec::new("rmape".into(), vec![Argument::Text("m".into())]),
        ])
        .unwrap();
    let mut plan = textforge::api::portable::PipelinePlan::new();
    for instruction in portable.get_transform_vec(&id).unwrap() {
        plan.add(instruction).unwrap();
    }
    let compiled = plan.compile().unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let processor = &compiled;
            scope.spawn(move || {
                for _ in 0..20 {
                    assert_eq!(processor.process("cat").unwrap(), "wolf");
                }
            });
        }
    });
}
#[cfg(feature = "test_access")]
#[test]
fn text_bytecode_and_file_batch_round_trip() {
    use textforge::api::portable::{FilePath, FileTask};
    let directory = tempfile::tempdir().unwrap();
    let path =
        |name: &str| FilePath::from_text(directory.path().join(name).to_str().unwrap().into());
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![
            InstructionSpec::new(
                "rmapd".into(),
                vec![
                    Argument::Text("animal map".into()),
                    Argument::Text("cat|🐈".into()),
                    Argument::Text("big wolf".into()),
                ],
            ),
            InstructionSpec::new("rmape".into(), vec![Argument::Text("animal map".into())]),
        ])
        .unwrap();
    for name in ["pipeline.textforge", "pipeline.textforgebc"] {
        std::fs::write(directory.path().join(name), "").unwrap();
    }
    processor
        .write_pipeline_to_file(&id, &path("pipeline.textforge"))
        .unwrap();
    let text_id = processor
        .get_pipeline_from_file(&path("pipeline.textforge"))
        .unwrap();
    processor
        .write_to_bytecode_file(&id, &path("pipeline.textforgebc"))
        .unwrap();
    let binary_id = processor
        .read_from_bytecode_file(&path("pipeline.textforgebc"))
        .unwrap();
    for id in [&text_id, &binary_id] {
        assert_eq!(
            processor.process_all(id, "cat 🐈").unwrap(),
            "big wolf big wolf"
        );
    }
    let mut tasks = Vec::new();
    for i in 0..8 {
        let input = format!("in{i}.txt");
        let output = format!("out{i}.txt");
        std::fs::write(directory.path().join(&input), "cat 🐈").unwrap();
        tasks.push(FileTask {
            input: path(&input),
            output: path(&output),
            pipeline_id: id.clone(),
        });
    }
    for result in processor.process_batch(tasks) {
        result.unwrap();
    }
    for i in 0..8 {
        assert_eq!(
            std::fs::read_to_string(directory.path().join(format!("out{i}.txt"))).unwrap(),
            "big wolf big wolf"
        );
    }
}

#[test]
fn definitions_are_rejected_inside_iterations() {
    let mut processor = PortableProcessor::new();
    let definition = InstructionSpec::new(
        "rmapd".into(),
        vec![
            Argument::Text("m".into()),
            Argument::Text("a".into()),
            Argument::Text("b".into()),
        ],
    );
    let result = processor.add_transform(vec![InstructionSpec::new(
        "iter".into(),
        vec![
            Argument::Number(2),
            Argument::Instruction(Box::new(definition)),
        ],
    )]);
    assert!(result.is_err());
}
