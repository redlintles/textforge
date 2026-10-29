#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

#[derive(Clone, Default)]
pub struct Tua {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Tua {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "tua"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "tua;\n".into()
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.is_empty() {
            return Ok(input);
        }
        Ok(input.to_uppercase().into())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x12
    }
    fn from_params(&mut self, params: &Vec<TextForgeParamTypes>) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "tua", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}
