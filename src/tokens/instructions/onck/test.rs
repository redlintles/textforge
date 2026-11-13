#[cfg(test)]
mod tests {
    use crate::{
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::onck::Onck, transforms::tua::Tua},
    };

    fn tua() -> TokenWrapper {
        (Box::new(Tua::default()) as Box<dyn InstructionMethods>).into()
    }

    #[test]
    fn transforms_only_the_section_without_a_context() {
        let token = Onck::new(1, 2, tua());
        assert_eq!(token.transform("abcd".into(), None).unwrap(), "aBCd");
    }

    #[test]
    fn string_repr_and_text_line() {
        let token = Onck::new(1, 2, tua());
        assert_eq!(token.get_string_repr(), "onck");
        assert!(token.to_textforge_line().starts_with("onck 1 2 apply tua"));
    }

    #[test]
    fn set_params_rejects_a_missing_instruction() {
        let mut token = Onck::default();
        let params: Vec<TextForgeParamTypes> = vec![1.into(), 2.into()];
        assert!(token.set_params(&params).is_err());
    }

    #[test]
    fn set_params_round_trips_get_params() {
        let source = Onck::new(1, 2, tua());
        let mut token = Onck::default();
        token.set_params(source.get_params()).unwrap();
        assert_eq!(token.transform("abcd".into(), None).unwrap(), "aBCd");
    }
}
