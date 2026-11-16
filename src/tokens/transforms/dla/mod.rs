#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::{check_index_against_input, check_vec_len};

use crate::utils::errors::TextForgeError;

/// `dla {index};` — Delete every character after `index`.
/// Delete all characters after `index` in the specified `input`
///
/// Returns a `TextForgeError` if `index` does not exist in `input`. At the last
/// character there is nothing after it, so the text is returned unchanged.
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dla::Dla};
///
/// let token = Dla::new(3);
///
/// assert_eq!(token.transform("banana orange red blue".into(),None).unwrap().to_string(), "bana")
///
/// ```
#[derive(Clone, Default)]
pub struct Dla {
    /// The index the instruction applies to.
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dla {
    /// Builds `dla {index};` with the given parameters.
    pub fn new(index: usize) -> Self {
        Dla {
            index,
            params: vec![index.into()],
        }
    }
}

impl InstructionMethods for Dla {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dla {};\n", self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_index_against_input(self.index, &input)?;

        // The last character has nothing after it: the text is unchanged
        // (Golden Rule 23: only an index that does not exist is an error).
        Ok(match input.char_indices().nth(self.index + 1) {
            Some((byte_index, _)) => crate::utils::allocation::keep(input, 0..byte_index),
            None => input,
        })
    }

    fn get_string_repr(&self) -> &'static str {
        "dla"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "dla", "")?;
        self.index = parse_args!(params, 0, Usize, "index");
        self.params = vec![self.index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x09
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.index),]);
        Ok(result)
    }
}
