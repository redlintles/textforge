#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;

use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::errors::TextForgeError};

/// `rev;` — Reverse the characters.
///
/// Reverses `input` character order. A `\r\n` line break is kept as `\r\n`
/// (reversed character by character it would become `\n\r`).
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rev::Rev};
///
/// let token = Rev::default();
/// assert_eq!(token.transform("foobar".into(),None).unwrap().to_string(), "raboof");
/// assert_eq!(token.transform("ab\r\ncd".into(),None).unwrap().to_string(), "dc\r\nba");
/// ``````
#[derive(Clone, Default)]
pub struct Rev {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Rev {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "rev"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        "rev;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.chars().nth(1).is_none() {
            return Ok(input);
        }
        if !input.contains('\r') {
            return Ok(input.chars().rev().collect());
        }
        // A `\r\n` line break stays `\r\n`: reversing it character by
        // character gave `\n\r`, which is not a line break on Windows and left
        // stray `\r` in the lines. Up to 0.3.1 it was reversed too.
        let mut output = String::with_capacity(input.len());
        let mut chars = input.chars().rev().peekable();
        while let Some(c) = chars.next() {
            if c == '\n' && chars.peek() == Some(&'\r') {
                chars.next();
                output.push_str("\r\n");
            } else {
                output.push(c);
            }
        }
        Ok(Cow::Owned(output))
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "rev", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x22
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}
