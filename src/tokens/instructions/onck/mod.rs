use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    parse_args,
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        sections::{apply_to_section, char_section},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;

/// Onck - On Chunk
///
/// Applies one instruction to characters `start..=end` (base 0, inclusive,
/// Unicode scalars) only. The instruction receives that slice, so every index
/// it uses is relative to it. As in `dlc`, an `end` past the last character is
/// clamped to it; a missing `start`, `start > end` or an empty document is an
/// error. `start == end` selects one character. Locating the chunk scans the
/// text once with `char_indices`.
///
/// DSL: `onck {start} {end} apply {instruction};`
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, instructions::onck::Onck, transforms::dlf::Dlf};
///
/// let token = Onck::new(1, 3, (Box::new(Dlf::default()) as Box<dyn InstructionMethods>).into());
/// assert_eq!(token.transform("abcde".into(), None).unwrap(), "acde");
/// ```
#[derive(Clone, Default)]
pub struct Onck {
    start_index: usize,
    end_index: usize,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl Onck {
    pub fn new(start_index: usize, end_index: usize, inner: TokenWrapper) -> Self {
        Onck {
            start_index,
            end_index,
            params: vec![
                start_index.into(),
                end_index.into(),
                TextForgeParamTypes::Token(inner.clone()),
            ],
            inner,
        }
    }
}

impl InstructionMethods for Onck {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x53
    }
    fn get_string_repr(&self) -> &'static str {
        "onck"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!(
            "onck {} {} apply {}",
            self.start_index,
            self.end_index,
            self.inner.to_textforge_line()
        ))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let range = char_section(&input, self.start_index, self.end_index, "onck")?;
        match context {
            Some(context) => apply_to_section(input, range, &self.inner, context),
            None => apply_to_section(
                input,
                range,
                &self.inner,
                &mut GlobalExecutionContext::new(),
            ),
        }
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 3, "onck", "param parsing error, invalid vec len")?;
        self.start_index = parse_args!(params, 0, Usize, "First argument should be an index");
        self.end_index = parse_args!(params, 1, Usize, "Second argument should be an index");
        self.inner = parse_args!(params, 2, Token, "Third argument should be an instruction");
        self.params = vec![
            self.start_index.into(),
            self.end_index.into(),
            TextForgeParamTypes::Token(self.inner.clone()),
        ];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        Ok(to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.start_index),
                TextForgeParamTypes::Usize(self.end_index),
                TextForgeParamTypes::Token(self.inner.clone()),
            ]
        ))
    }
}
