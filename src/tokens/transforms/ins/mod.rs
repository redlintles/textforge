#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::parser::params::TextForgeParamTypes;

use crate::utils::validations::check_vec_len;
use crate::{
    tokens::InstructionMethods,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};
/// `ins {index} {text};` — Insert `text` at the start (index 0) or after the character at `index`.
///
/// Index `0` inserts `text` at the start of `input`; any other index inserts
/// it after the character at that position (base 0). An index equal to the
/// number of characters appends.
///
/// If index does not exists in current string, `TextForgeError` is returned
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::ins::Ins};
///
/// let token = Ins::new(2,"orange");
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "banorangeana");
///
/// let at_start = Ins::new(0,"orange");
/// assert_eq!(at_start.transform("banana".into(),None).unwrap().to_string(), "orangebanana");
/// ```
#[derive(Clone, Default)]
pub struct Ins {
    index: usize,
    text_to_insert: String,
    params: Vec<TextForgeParamTypes>,
}

impl Ins {
    /// Builds `ins {index} {text};` with the given parameters.
    pub fn new(index: usize, text_to_insert: &str) -> Self {
        Ins {
            index,
            text_to_insert: text_to_insert.to_string(),
            params: vec![index.into(), text_to_insert.to_string().into()],
        }
    }
}
impl InstructionMethods for Ins {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "ins"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ins {} {};\n", self.index, self.text_to_insert).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        insert_after_char(input, self.index, &self.text_to_insert, || {
            self.to_textforge_line()
        })
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "ins", "")?;

        self.index = parse_args!(params, 0, Usize, "index");
        self.text_to_insert = parse_args!(params, 1, String, "text");

        self.params = vec![self.index.into(), self.text_to_insert.to_string().into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x28
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.index),
                TextForgeParamTypes::String(self.text_to_insert.clone()),
            ]
        );
        Ok(result)
    }
}

/// Insert `text` at the start of the text when `index` is 0, and after the
/// character at `index` (base 0) otherwise, as `ins` does. An index equal to
/// the character count appends; a larger one is an error. Shared with `inf`, so
/// both instructions follow exactly the same index rules.
pub(crate) fn insert_after_char<'a>(
    input: Cow<'a, str>,
    index: usize,
    text: &str,
    instruction: impl FnOnce() -> Cow<'static, str>,
) -> Result<Cow<'a, str>, TextForgeError> {
    if index > input.chars().count() {
        return Err(
            TextForgeError::new(
                TextForgeErrorCode::IndexOutOfRange(
                    format!(
                        "cannot insert after character {index}: the text has {} (insert after 0 to {}; the last appends)",
                        crate::utils::errors::plural(input.chars().count(), "character"),
                        input.chars().count()
                    ).into()
                ),
                instruction(),
                crate::utils::errors::input_excerpt(&input)
            )
        );
    }
    // Index 0 is the start of the text. Any other index inserts after the
    // character at that position. Before 0.3.2, index 0 inserted after the
    // first character, so the start of the text could not be reached.
    let byte_index = if index == 0 {
        0
    } else {
        input
            .char_indices()
            .nth(index + 1)
            .map(|(i, _)| i)
            .unwrap_or(input.len())
    };

    Ok(crate::utils::allocation::replace(
        input,
        byte_index..byte_index,
        text,
    ))
}
