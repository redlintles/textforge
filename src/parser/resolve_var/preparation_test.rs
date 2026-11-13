use super::*;
use crate::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
use crate::context::execution_context::{VarEntry, VarValues};
use crate::tokens::transforms::{atb::Atb, ate::Ate};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Counts parameter binding while executing an ordinary prefix transform.
#[derive(Clone)]
struct CountingPrefix {
    inner: Atb,
    binds: Arc<AtomicUsize>,
    opt_in: bool,
}
impl InstructionMethods for CountingPrefix {
    fn supports_preparation(&self) -> bool {
        self.opt_in
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        self.inner.get_params()
    }
    fn get_string_repr(&self) -> &'static str {
        "atb"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        self.inner.to_textforge_line()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        self.binds.fetch_add(1, Ordering::SeqCst);
        self.inner.set_params(params)
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        self.inner.transform(input, context)
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        self.inner.get_opcode()
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        self.inner.to_bytecode()
    }
}

#[test]
fn literal_binding_happens_once_but_custom_instructions_remain_opt_in() {
    for opt_in in [true, false] {
        let binds = Arc::new(AtomicUsize::new(0));
        let token = CountingPrefix {
            inner: Atb::default(),
            binds: Arc::clone(&binds),
            opt_in,
        };
        let wrapper = TokenWrapper::new(
            Box::new(token),
            Some(vec![ValType::Literal("!".to_string().into())]),
        );
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(vec![wrapper]);
        assert_eq!(binds.load(Ordering::SeqCst), usize::from(opt_in));
        for _ in 0..3 {
            assert_eq!(
                processor.run_transform(&id, Cow::Borrowed("x")).unwrap(),
                "!x"
            );
        }
        assert_eq!(binds.load(Ordering::SeqCst), if opt_in { 1 } else { 3 });
    }
}

#[test]
fn exported_clones_drop_cached_binding_before_public_template_mutation() {
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![TokenWrapper::new(
        Box::new(Atb::default()),
        Some(vec![ValType::Literal("!".to_string().into())]),
    )]);
    let mut exported = processor.get_transform_vec(&id).unwrap().remove(0);
    assert!(exported.prepared.is_none());
    exported.token = Box::new(Ate::default());
    assert_eq!(
        exported
            .apply_token(Cow::Borrowed("x"), &mut GlobalExecutionContext::new())
            .unwrap(),
        "x!"
    );
    assert_eq!(
        processor.run_transform(&id, Cow::Borrowed("x")).unwrap(),
        "!x"
    );
}

#[test]
fn variables_and_expressions_are_resolved_against_each_current_context() {
    for (source, expected) in [
        ("atb \"{{name}}\";", ["onex", "twox"]),
        ("atb \"[[ n + 1 ]]\";", ["3x", "10x"]),
        ("raw \"{{pattern}}\" X;", ["Xbc", "aXc"]),
    ] {
        let mut token = crate::text::reader::read_from_text(source).unwrap();
        token.prepare_constant();
        assert!(token.prepared.is_none());
        let mut context = GlobalExecutionContext::new();
        for (i, (name, n, pattern)) in [("one", 2, "a"), ("two", 9, "b")].into_iter().enumerate() {
            for (key, value) in [
                ("name", VarValues::String(name.into())),
                ("n", VarValues::Usize(n)),
                ("pattern", VarValues::String(pattern.into())),
            ] {
                context
                    .add_var(
                        key,
                        VarEntry {
                            value,
                            mutable: true,
                        },
                    )
                    .unwrap();
            }
            let input = if source.starts_with("raw") {
                "abc"
            } else {
                "x"
            };
            assert_eq!(
                token
                    .apply_token(Cow::Borrowed(input), &mut context)
                    .unwrap(),
                expected[i]
            );
        }
    }
}

#[test]
fn invalid_parameters_still_fail_only_when_the_instruction_executes() {
    let mut processor = TextForgeProcessor::new();
    let invalid = crate::text::reader::read_from_text("raw \"[\" X;").unwrap();
    let id = processor.add_transform(vec![
        crate::text::reader::read_from_text("end;").unwrap(),
        invalid.clone(),
    ]);
    assert_eq!(
        processor.run_transform(&id, Cow::Borrowed("abc")).unwrap(),
        "abc"
    );
    let id = processor.add_transform(vec![invalid]);
    assert!(processor.run_transform(&id, Cow::Borrowed("abc")).is_err());
}

fn counting_child(binds: &Arc<AtomicUsize>) -> TokenWrapper {
    TokenWrapper::new(
        Box::new(CountingPrefix {
            inner: Atb::default(),
            binds: Arc::clone(binds),
            opt_in: true,
        }),
        Some(vec![ValType::Literal("!".to_string().into())]),
    )
}

fn with_child(outer: &str, child: TokenWrapper) -> TokenWrapper {
    // Parse the outer instruction, then swap its nested instruction.
    let parsed = crate::text::reader::read_from_text(outer).unwrap();
    let mut params = parsed.get_params().clone();
    if let Some(last) = params.last_mut() {
        *last = ValType::Literal(TextForgeParamTypes::Token(child));
    }
    TokenWrapper::new(parsed.get_default_token(), Some(params))
}

#[test]
fn nested_instructions_are_bound_once_per_registration() {
    // Before, the nested instruction was re-bound on every iteration and every
    // execution (a regex child recompiled its pattern each time).
    for outer in [
        "iter 4 times null;",
        "onln 1 apply null;",
        "onlni 1 1 apply null;",
        "onck 0 0 apply null;",
        "ifdc x do null;",
        "onln '[[0 + 1]]' apply null;",
    ] {
        let binds = Arc::new(AtomicUsize::new(0));
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(vec![with_child(outer, counting_child(&binds))]);
        let after_registration = binds.load(Ordering::SeqCst);
        assert_eq!(after_registration, 1, "{outer}");
        for _ in 0..3 {
            processor.run_transform(&id, Cow::Borrowed("x")).unwrap();
        }
        assert_eq!(binds.load(Ordering::SeqCst), 1, "{outer}");
    }
}

#[test]
fn exported_nested_templates_carry_no_cached_binding() {
    let binds = Arc::new(AtomicUsize::new(0));
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![with_child(
        "iter 2 times null;",
        counting_child(&binds),
    )]);
    let exported = processor.get_transform_vec(&id).unwrap().remove(0);
    assert!(exported.prepared.is_none());
    for parameter in exported.get_params() {
        if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
            assert!(child.prepared.is_none());
        }
    }
}

#[test]
fn dynamic_nested_parameters_are_still_resolved_per_execution() {
    let mut processor = TextForgeProcessor::new();
    let tokens = [
        "iter 3 times ate '{{__COUNTER__}}';",
        "onln 1 apply ate '[[2 * 3]]';",
    ]
    .iter()
    .map(|line| crate::text::reader::read_from_text(line).unwrap())
    .collect();
    let id = processor.add_transform(tokens);
    for _ in 0..2 {
        assert_eq!(
            processor.run_transform(&id, Cow::Borrowed("x")).unwrap(),
            "x0126"
        );
    }
}
