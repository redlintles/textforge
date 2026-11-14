#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::validations::check_vec_len;
use crate::{tokens::InstructionMethods, utils::validations::check_chunk_bound_indexes};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

use crate::parser::params::TextForgeParamTypes;
/// `dlc {start} {end};` — Delete the characters `start..=end`.
///
/// Deletes an specific subslice of `input` delimited by `start_index` and `end_index`(inclusive)
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::dlc::Dlc};
///
/// let token = Dlc::new(1,5).unwrap();
///
/// assert_eq!(token.transform("bananaorangefullofjam".into(),None).unwrap().to_string(), "borangefullofjam")
///
/// ```
#[derive(Clone, Default)]
pub struct Dlc {
    /// First character or word of the inclusive range.
    pub start_index: usize,
    /// Last character or word of the inclusive range.
    pub end_index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Dlc {
    /// Builds `dlc {start} {end};` with the given parameters.
    ///
    /// Fails if the parameters are invalid (for a range, if `start > end`).
    pub fn new(start_index: usize, end_index: usize) -> Result<Self, TextForgeError> {
        check_chunk_bound_indexes(start_index, end_index, None)?;
        Ok(Dlc {
            start_index,
            end_index,
            params: vec![start_index.into(), end_index.into()],
        })
    }
}

impl InstructionMethods for Dlc {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("dlc {} {};\n", self.start_index, self.end_index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let len = input.chars().count();

        // deleting "everything" from an empty text gives an empty text
        let mut end = self.end_index;

        // ✅ clamp correto (inclusive range). Empty input wraps here and the
        // check below reports a missing start index, as in `slt` and `ctc`.
        if end >= len {
            end = len.wrapping_sub(1);
        }

        check_chunk_bound_indexes(self.start_index, end, Some(input.as_ref()))?;

        let start_index = input
            .char_indices()
            .nth(self.start_index)
            .map(|(i, _)| i)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::IndexOutOfRange(
                        format!(
                            "the range starts at character {}, but the text has {}",
                            self.start_index,
                            crate::utils::errors::plural(input.chars().count(), "character")
                        )
                        .into(),
                    ),
                    "",
                    crate::utils::errors::input_excerpt(&input),
                )
            })?;

        // end is now <= last_index, so end + 1 may equal len
        let end_index = input
            .char_indices()
            .nth(end + 1)
            .map(|(i, _)| i)
            .unwrap_or_else(|| input.len()); // when end is the last char, `after` starts at the end

        Ok(crate::utils::allocation::replace(
            input,
            start_index..end_index,
            "",
        ))
    }

    fn get_string_repr(&self) -> &'static str {
        "dlc"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "dlc", "")?;

        self.start_index = parse_args!(params, 0, Usize, "start");
        self.end_index = parse_args!(params, 1, Usize, "end");
        self.params = vec![self.start_index.into(), self.end_index.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x08
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.start_index),
                TextForgeParamTypes::Usize(self.end_index),
            ]
        );
        Ok(result)
    }
}
