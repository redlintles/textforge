#[cfg(test)]
mod tests {
    use crate::{
        context::execution_context::{
            GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
        },
        parser::{
            params::TextForgeParamTypes,
            resolve_var::{TokenWrapper, ValType},
        },
        tokens::{
            InstructionMethods,
            instructions::iter::Iter,
            transforms::{ate::Ate, dlf::Dlf},
        },
    };

    fn append_counter() -> TokenWrapper {
        TokenWrapper::new(
            Box::new(Ate::default()),
            Some(vec![ValType::Literal(TextForgeParamTypes::String(
                "[[__COUNTER__]]".into(),
            ))]),
        )
    }

    fn iter_of(times: usize, inner: TokenWrapper) -> Iter {
        let mut token = Iter::default();
        token
            .set_params(&[times.into(), TextForgeParamTypes::Token(inner)])
            .unwrap();
        token
    }

    #[test]
    fn counter_matches_the_current_iteration() {
        // Before the fix the body observed 0, 0, 1, 2: the counter was updated
        // only after applying the inner instruction.
        let token = iter_of(4, append_counter());
        let mut context = GlobalExecutionContext::new();
        let output = token.transform("".into(), Some(&mut context)).unwrap();
        assert_eq!(output, "0123");
    }

    #[test]
    fn counter_is_removed_after_the_loop() {
        let token = iter_of(2, append_counter());
        let mut context = GlobalExecutionContext::new();
        token.transform("".into(), Some(&mut context)).unwrap();
        assert!(context.get_var("__COUNTER__").is_err());
    }

    #[test]
    fn nested_iteration_restores_the_outer_counter() {
        // Before the fix the inner loop removed the shared variable and the outer
        // loop failed with VariableNotFound on its next update.
        let body = TokenWrapper::new(
            Box::new(Iter::default()),
            Some(vec![
                ValType::Literal(2.into()),
                ValType::Literal(TextForgeParamTypes::Token(append_counter())),
            ]),
        );
        let outer = iter_of(3, body);
        let mut context = GlobalExecutionContext::new();
        let output = outer.transform("".into(), Some(&mut context)).unwrap();
        assert_eq!(output, "010101");
        assert!(context.get_var("__COUNTER__").is_err());
    }

    #[test]
    fn outer_counter_is_visible_after_an_inner_loop() {
        // The body reads the outer counter after the inner loop has run.
        let (inner_loop, read_outer) = (
            TokenWrapper::new(
                Box::new(Iter::default()),
                Some(vec![
                    ValType::Literal(1.into()),
                    ValType::Literal(TextForgeParamTypes::Token(append_counter())),
                ]),
            ),
            append_counter(),
        );
        let mut context = GlobalExecutionContext::new();
        context
            .add_var(
                "__COUNTER__",
                VarEntry {
                    mutable: true,
                    value: VarValues::Usize(7),
                },
            )
            .unwrap();
        let inner = iter_of(1, inner_loop);
        let output = inner.transform("".into(), Some(&mut context)).unwrap();
        let output = read_outer.apply_token(output, &mut context).unwrap();
        assert_eq!(output, "07");
    }

    #[test]
    fn zero_iterations_return_input_unchanged() {
        let token = iter_of(0, Box::<Dlf>::default().into_wrapper());
        let mut context = GlobalExecutionContext::new();
        assert_eq!(
            token.transform("abc".into(), Some(&mut context)).unwrap(),
            "abc"
        );
    }

    #[test]
    fn missing_context_is_an_error() {
        let token = iter_of(1, append_counter());
        assert!(token.transform("abc".into(), None).is_err());
    }

    trait IntoWrapper {
        fn into_wrapper(self) -> TokenWrapper;
    }

    impl IntoWrapper for Box<Dlf> {
        fn into_wrapper(self) -> TokenWrapper {
            TokenWrapper::from(self as Box<dyn InstructionMethods>)
        }
    }

    #[cfg(feature = "bytecode")]
    mod bytecode {
        use super::*;
        use crate::bytecode::{reader::read_bytecode_from_file, writer::write_bytecode_to_file};

        #[test]
        fn to_bytecode_includes_both_parameters() {
            // Before the fix the instruction serialized with zero parameters.
            let body = crate::text::reader::read_from_text("ate x;").unwrap();
            let bytes = iter_of(3, body).to_bytecode().unwrap();
            assert_eq!(&bytes[8..12], &0x3c_u32.to_be_bytes());
            assert_eq!(bytes[12], 2);
        }

        #[test]
        fn file_round_trip_preserves_behavior() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("iter.textforgebc");
            std::fs::File::create(&path).unwrap();
            let token =
                crate::text::reader::read_from_text("iter 3 times ate '[[__COUNTER__]]';").unwrap();
            write_bytecode_to_file(&path, vec![token]).unwrap();
            let tokens = read_bytecode_from_file(&path).unwrap();
            let mut context = GlobalExecutionContext::new();
            let output = tokens[0].apply_token("".into(), &mut context).unwrap();
            assert_eq!(output, "012");
        }
    }
}
