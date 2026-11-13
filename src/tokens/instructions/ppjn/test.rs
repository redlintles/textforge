#[cfg(test)]
mod tests {
    use crate::{
        parser::{joins::JoinState, params::TextForgeParamTypes},
        tokens::{InstructionMethods, instructions::ppjn::Ppjn},
    };

    #[test]
    fn params_and_text_line() {
        let mut token = Ppjn::default();
        token
            .set_params(&[
                TextForgeParamTypes::String("lib/clean.textforge".into()),
                TextForgeParamTypes::Pipeline(JoinState::Unloaded),
            ])
            .unwrap();
        assert_eq!(token.to_textforge_line(), "ppjn lib/clean.textforge;\n");
        assert_eq!(token.get_string_repr(), "ppjn");
        assert!(
            token
                .set_params(&[TextForgeParamTypes::String("x".into())])
                .is_err()
        );
    }

    #[test]
    fn an_unloaded_join_reports_a_missing_file() {
        let mut token = Ppjn::default();
        token
            .set_params(&[
                TextForgeParamTypes::String("definitely-missing.textforge".into()),
                TextForgeParamTypes::Pipeline(JoinState::Unloaded),
            ])
            .unwrap();
        let error = token.transform("x".into(), None).unwrap_err();
        assert!(error.input.contains("definitely-missing.textforge"));
    }

    #[cfg(feature = "bytecode")]
    #[test]
    fn opcode() {
        assert_eq!(Ppjn::default().get_opcode(), 0x58);
    }
}
