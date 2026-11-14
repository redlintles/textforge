#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    };
    use crate::parser::params::TextForgeParamTypes;
    use crate::parser::resolve_var::{TokenWrapper, ValType};
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::val::Val;
    use crate::tokens::transforms::{rfw::Rfw, rpt::Rpt};
    use crate::utils::errors::TextForgeErrorCode;

    // ============================
    // Basic instruction contract
    // ============================

    #[test]
    fn get_string_repr_is_val() {
        let t = Val::default();
        assert_eq!(t.get_string_repr(), "val");
    }

    #[test]
    fn set_params_sets_name_and_string_value() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());

        let stored = t.get_params();
        assert!(matches!(stored.first(), Some(TextForgeParamTypes::String(s)) if s == "n"));
        assert!(matches!(stored.get(1), Some(TextForgeParamTypes::String(s)) if s == "5"));
    }

    #[test]
    fn set_params_accepts_varref_as_value() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Val::default();
        let params = vec![TextForgeParamTypes::String("n".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_non_string_name() {
        let mut t = Val::default();
        let params = vec![
            TextForgeParamTypes::Usize(1),
            TextForgeParamTypes::String("5".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidParameters(_)
        ));
    }

    // ============================
    // Bugs found while writing this suite (unrelated to type inference,
    // caught by checking Val::default() against the grammar in table.rs)
    // ============================

    #[test]
    fn default_params_should_match_val_syntax_but_currently_dont() {
        // The `val` syntax in table.rs declares both slots as
        // SyntaxToken::String. `Val::default()`, however, put a `Token` in slot 1
        // (instead of String), inconsistent with its own `val_value` (String(""))
        // and with what `resolve_variables` requires. A `Val::default()`
        // wrapped in a TokenWrapper without going through `set_params` first
        // (e.g. `TokenWrapper::new(Box::new(Val::default()), None)`) then broke
        // at run time with IncompatibleTypeError. This documents the expected
        // format.
        let t = Val::default();
        let params = t.get_params();

        assert!(
            matches!(params.get(1), Some(TextForgeParamTypes::String(_))),
            "expected slot[1] = String (matching val_value and the grammar), got {:?}",
            params.get(1)
        );
    }

    #[test]
    fn to_textforge_line_should_be_reparseable_but_is_missing_the_terminator() {
        // Every other instruction ends to_textforge_line() with ";\n" (see rpt:
        // "rpt {};\n", rfw: "rfw {} {};\n") because read_from_text expects a ';'
        // at the end of the line (strip_suffix(";") in text/reader.rs). Val::to_textforge_line()
        // did not add that suffix, so serializing a pipeline with `val` and reading it back
        // through read_from_text failed. This documents the expected format.
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        assert_eq!(t.to_textforge_line().as_ref(), "val n = 5;\n");
    }

    // ============================
    // transform(): declaring a variable in the context
    // ============================

    #[test]
    fn transform_declares_immutable_string_variable() {
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        let mut ctx = GlobalExecutionContext::new();
        let result = t
            .transform("input inalterado".into(), Some(&mut ctx))
            .unwrap();

        assert_eq!(result.to_string(), "input inalterado");

        let var = ctx
            .get_var("n")
            .expect("variable 'n' should exist in the context");
        assert!(matches!(&var.value, VarValues::String(s) if s == "5"));
        assert!(
            !var.mutable,
            "val must always declare an immutable variable"
        );
    }

    #[test]
    fn transform_aliases_existing_variable_via_varref() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "x",
            VarEntry {
                value: VarValues::String("hi".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ])
        .unwrap();

        t.transform("qualquer coisa".into(), Some(&mut ctx))
            .unwrap();

        let y = ctx
            .get_var("y")
            .expect("'y' should have been created from 'x'");
        assert!(matches!(&y.value, VarValues::String(s) if s == "hi"));
    }

    #[test]
    fn transform_fails_without_context() {
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), None).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
    }

    #[test]
    fn transform_fails_when_varref_source_is_missing() {
        let mut ctx = GlobalExecutionContext::new();
        let mut t = Val::default();
        t.set_params(&[
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("inexistente".to_string()),
        ])
        .unwrap();

        let err = t.transform("input".into(), Some(&mut ctx)).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::VariableNotFound(_)
        ));
    }

    // ============================
    // resolve_variables(): type coercion between a variable and the slot that uses it.
    // String->Usize (success and failure) plus the sibling cases
    // (Usize->String, and the case with no possible conversion: Token).
    //
    // These exercise resolve_variables, not Val itself; they could move to
    // a test module of their own.
    // ============================

    #[test]
    fn resolve_variables_coerces_string_var_into_usize_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("5".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rpt::default()),
            Some(vec![ValType::VarRef("n".to_string())]),
        );

        let result = wrapper.apply_token(Cow::from("ab"), &mut ctx).unwrap();

        assert_eq!(result.to_string(), "ababababab");
    }

    #[test]
    fn resolve_variables_errors_when_string_var_is_not_numeric() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("abc".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rpt::default()),
            Some(vec![ValType::VarRef("n".to_string())]),
        );

        let err = wrapper.apply_token(Cow::from("ab"), &mut ctx).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::IncompatibleTypeError(_)
        ));
    }

    #[test]
    fn resolve_variables_coerces_usize_var_into_string_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::Usize(7),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rfw::default()),
            Some(vec![
                ValType::VarRef("n".to_string()),
                ValType::Literal(TextForgeParamTypes::String("X".to_string())),
            ]),
        );

        let result = wrapper
            .apply_token(Cow::from("say 7 twice 7"), &mut ctx)
            .unwrap();

        assert_eq!(result.to_string(), "say X twice 7".to_string());
    }

    #[test]
    fn resolve_variables_rejects_token_var_in_string_slot() {
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "tok",
            VarEntry {
                value: VarValues::Token(TokenWrapper::default()),
                mutable: false,
            },
        )
        .unwrap();

        let wrapper = TokenWrapper::new(
            Box::new(Rfw::default()),
            Some(vec![
                ValType::VarRef("tok".to_string()),
                ValType::Literal(TextForgeParamTypes::String("X".to_string())),
            ]),
        );

        let err = wrapper
            .apply_token(Cow::from("input"), &mut ctx)
            .unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::IncompatibleTypeError(_)
        ));
    }
}

#[cfg(all(test, feature = "bytecode"))]
mod opcode_regression {
    use crate::{
        globals::table::{QuerySource, QueryTarget, TOKEN_TABLE, TargetValue},
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::val::Val},
    };

    #[test]
    fn val_opcode_matches_registry_and_survives_parameter_roundtrip() {
        let mut token = Val::default();
        token
            .set_params(&["prefix".to_string().into(), "🍌".to_string().into()])
            .unwrap();
        let opcode = match TOKEN_TABLE
            .find((QuerySource::Identifier("val".into()), QueryTarget::Bytecode))
            .unwrap()
        {
            TargetValue::Bytecode(opcode) => opcode,
            _ => panic!("expected opcode"),
        };
        assert_eq!(token.get_opcode(), opcode);
        assert_eq!(opcode, 0x36);
        let parameter = TextForgeParamTypes::Token(TokenWrapper::new(Box::new(token), None));
        let (_, bytes) = parameter
            .param_to_bytecode(
                &mut crate::context::execution_context::GlobalExecutionContext::new(),
            )
            .unwrap();
        match TextForgeParamTypes::from_bytecode(bytes).unwrap() {
            TextForgeParamTypes::Token(wrapper) => assert_eq!(wrapper.get_string_repr(), "val"),
            _ => panic!("expected val token"),
        }
    }
}
