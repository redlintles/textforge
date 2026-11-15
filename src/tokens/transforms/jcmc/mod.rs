#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;

/// `jcmc;` — Join the words as camelCase.
///
/// If `input` is a string whose words are separated by spaces, join `input` as a camelCase string
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::jcmc::Jcmc};
///
/// let token = Jcmc::default();
///
/// assert_eq!(token.transform("banana orange full of jam".into(),None).unwrap().to_string(), "bananaOrangeFullOfJam");
/// // The first letter is lowercased; the other letters of each word are kept.
/// assert_eq!(token.transform("Hello World".into(),None).unwrap().to_string(), "helloWorld");
/// assert_eq!(token.transform("Élan vital".into(),None).unwrap().to_string(), "élanVital");
/// ```
///
#[derive(Clone, Default)]
pub struct Jcmc {
    params: Vec<TextForgeParamTypes>,
}

impl InstructionMethods for Jcmc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "jcmc"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        "jcmc;\n".into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = String::with_capacity(input.len());
        for (i, word) in input.split_whitespace().enumerate() {
            if i == 0 {
                // camelCase starts with a lowercase letter. Before 0.3.2 the
                // first word was copied, so "Hello World" gave "HelloWorld".
                let mut chars = word.chars();
                if let Some(first) = chars.next() {
                    result.extend(first.to_lowercase());
                    result.push_str(chars.as_str());
                }
            } else {
                crate::utils::allocation::capitalize_into(&mut result, word);
            }
        }
        Ok(result.into())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x2d
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 0, "jcmc", "")?;
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(self.get_opcode(), []);
        Ok(result)
    }
}
