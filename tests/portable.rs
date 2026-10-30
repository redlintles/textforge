//! Portable API tests deliberately use public production paths only.
//! No `test_access`, parser types, or internal tree access are needed.
use textforge::api::{
    TextForgeBlockMethods, TextForgeBuilderMethods, TextForgeConditionalMethods,
    portable::{
        Argument, BindingError, CompiledPipeline, ErrorStage, InstructionSpec, PipelinePlan,
    },
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};

fn operation(name: &str, arguments: Vec<Argument>) -> InstructionSpec {
    InstructionSpec::new(name.to_string(), arguments)
}

#[test]
fn portable_output_matches_fluent_rust_api_for_unicode_and_empty_inputs() {
    let mut plan = PipelinePlan::new();
    plan.add(operation("tbs", vec![])).unwrap();
    plan.add(operation("atb", vec![Argument::Text("🍌".into())]))
        .unwrap();
    plan.add(operation("ate", vec![Argument::Text("e\u{301}".into())]))
        .unwrap();
    let compiled = plan.compile().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .trim_both_sides()
        .unwrap()
        .add_to_beginning("🍌")
        .unwrap()
        .add_to_end("e\u{301}")
        .unwrap()
        .build()
        .unwrap();
    for input in ["", " x ", " é ", " 👩‍💻 ", " e\u{301} ", "a\0b"] {
        assert_eq!(
            compiled.process(input).unwrap(),
            processor.process_all(&id, input).unwrap()
        );
    }
}

#[test]
fn dynamic_block_names_and_subplans_have_independent_lifetimes() {
    let name = format!("bloco-{}-🍌", 42);
    let compiled = {
        let mut body = PipelinePlan::new();
        body.add(operation("atb", vec![Argument::Text("🍌".into())]))
            .unwrap();
        let mut plan = PipelinePlan::new();
        plan.block(name.clone(), body);
        plan.call_block(name).unwrap();
        plan.compile().unwrap()
    };
    assert_eq!(compiled.process("é").unwrap(), "🍌é");
    assert_eq!(compiled.process("").unwrap(), "🍌");
}

#[test]
fn compilation_is_repeatable_and_empty_plan_is_identity() {
    let plan = PipelinePlan::default();
    let first = plan.compile().unwrap();
    let second = plan.compile().unwrap();
    drop(plan);
    assert_eq!(first.process("🍌").unwrap(), "🍌");
    assert_eq!(second.process("e\u{301}").unwrap(), "e\u{301}");
}

#[test]
fn references_numbers_and_text_interpolation_match_the_rust_api() {
    let mut plan = PipelinePlan::new();
    // DSL val/var values are strings, just as the fluent helpers normalize them.
    plan.add(operation(
        "val",
        vec![Argument::Text("n".into()), Argument::Text("2".into())],
    ))
    .unwrap();
    plan.add(operation("rpt", vec![Argument::Variable("n".into())]))
        .unwrap();
    plan.add(operation("ate", vec![Argument::Text("[[ 2 + 3 ]]".into())]))
        .unwrap();
    plan.add(operation("rpt", vec![Argument::Number(2)]))
        .unwrap();
    let compiled = plan.compile().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .val("n", "2")
        .unwrap()
        .repeat(textforge::api::get_var("n"))
        .unwrap()
        .add_to_end("[[ 2 + 3 ]]")
        .unwrap()
        .repeat(2_usize)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(
        compiled.process("é").unwrap(),
        processor.process_all(&id, "é").unwrap()
    );
    assert_eq!(compiled.process("x").unwrap(), "xx5xx5");
}

#[test]
fn conditional_groups_recheck_the_condition_for_each_child() {
    let mut body = PipelinePlan::new();
    body.add(operation("rmp", vec![Argument::Text("x".into())]))
        .unwrap();
    body.add(operation(
        "ate",
        vec![Argument::Text("must not run".into())],
    ))
    .unwrap();
    let mut plan = PipelinePlan::new();
    plan.contains_each("x".into(), body);
    let compiled = plan.compile().unwrap();
    assert_eq!(compiled.process("x").unwrap(), "");
    assert_eq!(compiled.process("é").unwrap(), "é");
}

#[test]
fn blocks_with_conditionals_reuse_the_same_hierarchy_semantics() {
    let mut conditional = PipelinePlan::new();
    conditional
        .add(operation("atb", vec![Argument::Text("🍌".into())]))
        .unwrap();
    conditional
        .add(operation("ate", vec![Argument::Text("é".into())]))
        .unwrap();
    let mut body = PipelinePlan::new();
    body.contains_each("x".into(), conditional);
    let mut plan = PipelinePlan::new();
    plan.block("outer".into(), body);
    plan.call_block("outer".into()).unwrap();
    let compiled = plan.compile().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .block_assoc("outer", |body| {
            body.if_do_contains_each("x", |conditional| {
                conditional.add_to_beginning("🍌")?.add_to_end("é")?;
                Ok(())
            })?;
            Ok(())
        })
        .unwrap()
        .call_block("outer")
        .unwrap()
        .build()
        .unwrap();
    for input in ["", "x", "é", "e\u{301}x"] {
        assert_eq!(
            compiled.process(input).unwrap(),
            processor.process_all(&id, input).unwrap()
        );
    }
}

#[test]
fn nested_instruction_arguments_execute_without_callbacks() {
    let mut plan = PipelinePlan::new();
    plan.add(operation(
        "iter",
        vec![
            Argument::Number(2),
            Argument::Instruction(Box::new(operation(
                "atb",
                vec![Argument::Text("🍌".into())],
            ))),
        ],
    ))
    .unwrap();
    assert_eq!(plan.compile().unwrap().process("é").unwrap(), "🍌🍌é");
}

#[test]
fn failed_conversion_leaves_existing_plan_unchanged_and_errors_are_uncolored() {
    let mut plan = PipelinePlan::new();
    plan.add(operation("atb", vec![Argument::Text("keep".into())]))
        .unwrap();
    let error = plan
        .add(operation("not_an_instruction", vec![]))
        .unwrap_err();
    assert_eq!(error.stage, ErrorStage::Conversion);
    assert_eq!(error.code, 2);
    assert_eq!(error.instruction, "not_an_instruction");
    assert!(!error.message.contains('\u{1b}'));
    assert!(!error.to_string().contains('\u{1b}'));
    assert_eq!(plan.compile().unwrap().process("x").unwrap(), "keepx");
}

#[test]
fn malformed_parameter_shapes_fail_at_compile_without_mutating_the_plan() {
    for arguments in [
        vec![],
        vec![Argument::Number(2)],
        vec![Argument::Text("a".into()), Argument::Text("b".into())],
    ] {
        let mut plan = PipelinePlan::new();
        plan.add(operation("atb", arguments)).unwrap();
        let first = plan.compile().unwrap_err();
        assert_eq!(first.stage, ErrorStage::Validation);
        assert_eq!(first, plan.compile().unwrap_err());
    }
}

#[test]
fn nested_blocks_are_rejected_at_any_ancestor_distance() {
    for conditional in [false, true] {
        let mut leaf = PipelinePlan::new();
        leaf.add(operation("tbs", vec![])).unwrap();
        let mut inner = PipelinePlan::new();
        inner.block("inner".into(), leaf);
        let body = if conditional {
            let mut wrapper = PipelinePlan::new();
            wrapper.contains_each("x".into(), inner);
            wrapper
        } else {
            inner
        };
        let mut plan = PipelinePlan::new();
        plan.block("outer".into(), body);
        let error = plan.compile().unwrap_err();
        assert_eq!(error.stage, ErrorStage::Validation);
        assert_eq!(error.code, 28);
    }
}

#[test]
fn raw_nested_block_instructions_cannot_bypass_validation() {
    let nested = operation(
        "blk",
        vec![
            Argument::Text("inner".into()),
            Argument::Instruction(Box::new(operation("tbs", vec![]))),
        ],
    );
    let mut body = PipelinePlan::new();
    body.add(operation(
        "ifdc",
        vec![
            Argument::Text("x".into()),
            Argument::Instruction(Box::new(nested)),
        ],
    ))
    .unwrap();
    let mut plan = PipelinePlan::new();
    plan.block("outer".into(), body);
    assert_eq!(plan.compile().unwrap_err().code, 28);
}

#[test]
fn depth_limits_apply_to_combined_groups_and_instruction_arguments() {
    let mut body = PipelinePlan::new();
    body.add(operation(
        "ifdc",
        vec![
            Argument::Text("x".into()),
            Argument::Instruction(Box::new(operation("tbs", vec![]))),
        ],
    ))
    .unwrap();
    let mut plan = PipelinePlan::new();
    plan.contains_each("x".into(), body);
    assert_eq!(plan.compile().unwrap_err().code, 20);
}

#[test]
fn undefined_variables_and_blocks_remain_execution_errors() {
    let mut plan = PipelinePlan::new();
    plan.add(operation("atb", vec![Argument::Variable("missing".into())]))
        .unwrap();
    let compiled = plan.compile().unwrap();
    let error = compiled.process("é").unwrap_err();
    assert_eq!(error.stage, ErrorStage::Execution);
    assert_eq!(error.code, 9);
    assert_eq!(compiled.process("é").unwrap_err(), error);
    let mut plan = PipelinePlan::new();
    plan.call_block("missing".into()).unwrap();
    assert_eq!(plan.compile().unwrap().process("x").unwrap_err().code, 8);
}

#[test]
fn owned_compiled_pipeline_can_be_shared_for_isolated_rust_execution() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<CompiledPipeline>();
    let mut plan = PipelinePlan::new();
    plan.add(operation("atb", vec![Argument::Text("🍌".into())]))
        .unwrap();
    let pipeline = std::sync::Arc::new(plan.compile().unwrap());
    let handles = ["x", "é", "", "e\u{301}"].map(|input| {
        let pipeline = pipeline.clone();
        std::thread::spawn(move || pipeline.process(input).unwrap())
    });
    for (handle, expected) in handles.into_iter().zip(["🍌x", "🍌é", "🍌", "🍌e\u{301}"]) {
        assert_eq!(handle.join().unwrap(), expected);
    }
}

#[test]
fn excessive_owned_hierarchies_are_rejected_and_released_without_recursive_walks() {
    let mut body = PipelinePlan::new();
    for _ in 0..10_000 {
        let mut parent = PipelinePlan::new();
        parent.contains_each("x".into(), body);
        body = parent;
    }
    assert_eq!(body.compile().unwrap_err().stage, ErrorStage::Validation);
    drop(body);
    let mut specification = operation("tbs", vec![]);
    for _ in 0..10_000 {
        specification = operation(
            "ifdc",
            vec![
                Argument::Text("x".into()),
                Argument::Instruction(Box::new(specification)),
            ],
        );
    }
    let mut plan = PipelinePlan::new();
    assert_eq!(
        plan.add(specification).unwrap_err().stage,
        ErrorStage::Conversion
    );
    assert_eq!(
        plan.compile().unwrap().process("identity").unwrap(),
        "identity"
    );
}

#[test]
fn native_number_conversion_never_truncates() {
    let mut plan = PipelinePlan::new();
    let result = plan.add(operation("dls", vec![Argument::Number(u64::MAX)]));
    if usize::try_from(u64::MAX).is_ok() {
        result.unwrap();
    } else {
        let error: BindingError = result.unwrap_err();
        assert_eq!(error.stage, ErrorStage::Conversion);
        assert_eq!(error.code, 22);
    }
}

#[test]
fn runtime_regex_failures_are_structured_execution_errors() {
    let mut plan = PipelinePlan::new();
    plan.add(operation("rmp", vec![Argument::Text("[".into())]))
        .unwrap();
    let error = plan.compile().unwrap().process("é").unwrap_err();
    assert_eq!(error.stage, ErrorStage::Execution);
    assert!(!error.message.is_empty());
}
