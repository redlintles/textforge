#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// TBS - Trim both sides
///
/// Trim Both Sides of `input`, removing all spaces from both the beginning and the end
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::tbs::Tbs};
///
/// let token = Tbs::default();
///
/// assert_eq!(token.transform("   banana   ".into(),None).unwrap().to_string(), "banana");
/// ```
///
#[derive(Clone, Default)]
pub struct Tbs {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Tbs {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "tbs;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.is_empty() {
            return Ok(input);
        }
        Ok(String::from(input.trim()).into())
    }

    fn get_string_repr(&self) -> &'static str {
        "tbs"
    }
    fn from_params(&mut self, params: &Vec<TextForgeParamTypes>) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "tbs", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x05
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}
