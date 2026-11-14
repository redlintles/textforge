#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use crate::context::execution_context::{
        GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues,
    };
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::var::Var;
    use crate::utils::errors::TextForgeErrorCode;

    // ============================
    // Basic instruction contract
    // ============================

    #[test]
    fn get_string_repr_is_var() {
        let t = Var::default();
        assert_eq!(t.get_string_repr(), "var");
    }

    #[test]
    fn set_params_sets_name_and_string_value() {
        let mut t = Var::default();
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
        let mut t = Var::default();
        let params = vec![
            TextForgeParamTypes::String("y".to_string()),
            TextForgeParamTypes::VarRef("x".to_string()),
        ];

        assert!(t.set_params(&params).is_ok());
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Var::default();
        let params = vec![TextForgeParamTypes::String("n".to_string())];

        let err = t.set_params(&params).unwrap_err();
        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_non_string_name() {
        let mut t = Var::default();
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
    // Regression: unlike Val::default(), Var::default() matches the
    // grammar (slot[1] = String) and to_textforge_line includes ";\n".
    // Kept as positive tests to lock the correct behavior in.
    // ============================

    #[test]
    fn default_params_match_var_syntax() {
        let t = Var::default();
        let params = t.get_params();

        assert!(matches!(
            params.get(1),
            Some(TextForgeParamTypes::String(_))
        ));
    }

    #[test]
    fn to_textforge_line_is_reparseable() {
        let mut t = Var::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("5".to_string()),
        ])
        .unwrap();

        assert_eq!(t.to_textforge_line().as_ref(), "var n = 5;\n");
    }

    // ============================
    // transform(): declaring a variable in the context
    // ============================

    #[test]
    fn transform_declares_mutable_string_variable() {
        let mut t = Var::default();
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
        assert!(var.mutable, "var must always declare a mutable variable");
    }

    #[test]
    fn transform_declares_mutable_usize_variable() {
        let mut t = Var::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::Usize(7),
        ])
        .unwrap();

        let mut ctx = GlobalExecutionContext::new();
        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let var = ctx.get_var("n").unwrap();
        assert!(matches!(&var.value, VarValues::Usize(7)));
        assert!(var.mutable);
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

        let mut t = Var::default();
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
        // aliasing through var always makes the copy mutable, whatever the
        // mutability of the source variable (x is immutable, y is not).
        assert!(y.mutable);
    }

    #[test]
    fn transform_fails_without_context() {
        let mut t = Var::default();
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
        let mut t = Var::default();
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

    #[test]
    fn transform_redeclaring_existing_name_overwrites_it() {
        // add_var inserts unconditionally, so `var` on a name already
        // used by a `val` (immutable) simply overwrites it
        // and the new entry is mutable.
        let mut ctx = GlobalExecutionContext::new();
        ctx.add_var(
            "n",
            VarEntry {
                value: VarValues::String("old".to_string()),
                mutable: false,
            },
        )
        .unwrap();

        let mut t = Var::default();
        t.set_params(&[
            TextForgeParamTypes::String("n".to_string()),
            TextForgeParamTypes::String("new".to_string()),
        ])
        .unwrap();

        t.transform("input".into(), Some(&mut ctx)).unwrap();

        let n = ctx.get_var("n").unwrap();
        assert!(matches!(&n.value, VarValues::String(s) if s == "new"));
        assert!(n.mutable);
    }
}
