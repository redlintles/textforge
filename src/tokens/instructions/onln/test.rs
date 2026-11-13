#[cfg(test)]
mod tests {
    use crate::{
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::onln::Onln, transforms::tua::Tua},
    };

    fn tua() -> TokenWrapper {
        (Box::new(Tua::default()) as Box<dyn InstructionMethods>).into()
    }

    #[test]
    fn transforms_only_the_section_without_a_context() {
        let token = Onln::new(2, tua());
        assert_eq!(token.transform("a\nb\n".into(), None).unwrap(), "a\nB\n");
    }

    #[test]
    fn string_repr_and_text_line() {
        let token = Onln::new(2, tua());
        assert_eq!(token.get_string_repr(), "onln");
        assert!(token.to_textforge_line().starts_with("onln 2 apply tua"));
    }

    #[test]
    fn set_params_rejects_a_missing_instruction() {
        let mut token = Onln::default();
        let params: Vec<TextForgeParamTypes> = vec![1.into()];
        assert!(token.set_params(&params).is_err());
    }

    #[test]
    fn set_params_round_trips_get_params() {
        let source = Onln::new(2, tua());
        let mut token = Onln::default();
        token.set_params(source.get_params()).unwrap();
        assert_eq!(token.transform("a\nb\n".into(), None).unwrap(), "a\nB\n");
    }
}
