use textforge::api::{
    TextForgeBlockMethods, TextForgeBuilderMethods,
    portable::{Argument, InstructionSpec, PipelinePlan},
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};

#[test]
fn scoped_map_keeps_order_and_end_does_not_halt_the_pipeline() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .define_replace_map("animals", |map| {
            map.add_replace("cat", "wolf")?
                .add_replace("wolf", "dog")?
                .end()
        })
        .unwrap()
        .execute_replace_map("animals")
        .unwrap()
        .add_to_end("!")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all(&id, "cat wolf").unwrap(), "wolf dog!");
    assert_eq!(processor.process_all(&id, "cat wolf").unwrap(), "wolf dog!");
}

#[test]
fn failed_or_unfinished_configuration_appends_nothing() {
    let mut processor = TextForgeProcessor::new();
    let mut plan = processor.create_pipeline();
    plan.add_to_end("!").unwrap();
    assert!(
        plan.define_replace_map("unfinished", |map| {
            map.add_replace("a", "b")?;
            Ok(())
        })
        .is_err()
    );
    assert!(plan.define_replace_map("empty", |map| map.end()).is_err());
    assert!(
        plan.define_replace_map("closed", |map| {
            map.add_replace("a", "b")?.end()?;
            map.add_replace("c", "d")?;
            Ok(())
        })
        .is_err()
    );
    let id = plan.build().unwrap();
    assert_eq!(processor.get_transform_vec(&id).unwrap().len(), 1);
    assert_eq!(processor.process_all(&id, "a").unwrap(), "a!");
    let mut builder = processor.create_pipeline();
    assert!(
        builder
            .define_replace_map("double", |map| {
                map.add_replace("a", "b")?.end()?;
                map.end()
            })
            .is_err()
    );
}

#[test]
fn portable_definition_validates_before_appending_and_preserves_order() {
    let mut plan = PipelinePlan::new();
    assert!(
        plan.define_replace_map(Argument::Text("empty".into()), vec![])
            .is_err()
    );
    plan.define_replace_map(
        Argument::Text("m".into()),
        vec![
            (Argument::Text("a".into()), Argument::Text("b".into())),
            (Argument::Text("b".into()), Argument::Text("c".into())),
        ],
    )
    .unwrap();
    plan.add(InstructionSpec::new(
        "rmape".into(),
        vec![Argument::Text("m".into())],
    ))
    .unwrap();
    assert_eq!(plan.compile().unwrap().process("ab").unwrap(), "bc");
}

#[test]
fn debug_matches_normal_execution_for_the_reported_block_and_map_pipeline() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .trim_both_sides()
        .unwrap()
        .add_to_beginning("Banana ")
        .unwrap()
        .add_to_end("!")
        .unwrap()
        .block_assoc("banana", |body| {
            body.to_reverse()?
                .to_lowercase_all()?
                .replace_all_with("a", "o")?;
            Ok(())
        })
        .unwrap()
        .normalize_unicode("nfc")
        .unwrap()
        .call_block("banana")
        .unwrap()
        .add_to_end("banana laranja fogo vermelho do inferno")
        .unwrap()
        .define_replace_map("rmap1", |map| {
            map.add_replace("a", "b")?
                .add_replace("o", "x")?
                .add_replace("e", "t")?
                .end()
        })
        .unwrap()
        .execute_replace_map("rmap1")
        .unwrap()
        .build()
        .unwrap();
    let expected = processor.process_all(&id, "Fruit").unwrap();
    assert!(expected.contains("fxgx"));
    assert_eq!(
        processor.process_all_with_debug(&id, "Fruit").unwrap(),
        expected
    );
    assert_eq!(
        processor.process_all_with_debug(&id, "Fruit").unwrap(),
        expected
    );
}

#[test]
fn debug_uses_resolved_block_names_including_whitespace() {
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .val("name", "actual block")
        .unwrap()
        .block_assoc("{{name}}", |body| {
            body.add_to_end("🍌")?;
            Ok(())
        })
        .unwrap()
        .call_block("{{name}}")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(processor.process_all_with_debug(&id, "é").unwrap(), "é🍌");
}

#[cfg(feature = "test_access")]
#[test]
fn block_and_map_exports_round_trip_without_losing_quoted_parameters() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .block_assoc("actual block", |body| {
            body.replace_all_with("a", "o")?
                .replace_first_with("o", "a")?;
            Ok(())
        })
        .unwrap()
        .call_block("actual block")
        .unwrap()
        .add_to_end(" banana fruit")
        .unwrap()
        .define_replace_map("my map", |map| map.add_replace("fruit", "🍌 fruit")?.end())
        .unwrap()
        .execute_replace_map("my map")
        .unwrap()
        .build()
        .unwrap();
    let expected = processor.process_all(&id, "banana").unwrap();
    let text = directory.path().join("plan.textforge");
    let bytecode = directory.path().join("plan.textforgebc");
    std::fs::File::create(&text).unwrap();
    std::fs::File::create(&bytecode).unwrap();
    processor.write_pipeline_to_file(&id, &text).unwrap();
    processor.write_to_bytecode_file(&id, &bytecode).unwrap();
    for loaded in [
        processor.get_pipeline_from_file(&text).unwrap(),
        processor.read_from_bytecode_file(&bytecode).unwrap(),
    ] {
        assert_eq!(
            processor.process_all_with_debug(&loaded, "banana").unwrap(),
            expected
        );
    }
}
