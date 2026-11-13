#[cfg(test)]
mod tests {
    use crate::{
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::onlni::Onlni, transforms::tua::Tua},
    };

    fn tua() -> TokenWrapper {
        (Box::new(Tua::default()) as Box<dyn InstructionMethods>).into()
    }

    #[test]
    fn transforms_only_the_section_without_a_context() {
        let token = Onlni::new(2, 3, tua());
        assert_eq!(token.transform("a\nb\nc".into(), None).unwrap(), "a\nB\nC");
    }

    #[test]
    fn string_repr_and_text_line() {
        let token = Onlni::new(2, 3, tua());
        assert_eq!(token.get_string_repr(), "onlni");
        assert!(token.to_textforge_line().starts_with("onlni 2 3 apply tua"));
    }

    #[test]
    fn set_params_rejects_a_missing_instruction() {
        let mut token = Onlni::default();
        let params: Vec<TextForgeParamTypes> = vec![1.into(), 2.into()];
        assert!(token.set_params(&params).is_err());
    }

    #[test]
    fn set_params_round_trips_get_params() {
        let source = Onlni::new(2, 3, tua());
        let mut token = Onlni::default();
        token.set_params(source.get_params()).unwrap();
        assert_eq!(token.transform("a\nb\nc".into(), None).unwrap(), "a\nB\nC");
    }
}
