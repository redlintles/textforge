use std::borrow::Cow;

use crate::{
    context::{debug_trace::within, execution_context::GlobalExecutionContext},
    parse_args,
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        sections::{apply_to_section, line_section},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;

/// Onlni - On Line Interval
///
/// Applies one instruction to lines `start..=end` (base 1, inclusive) as a
/// single block. The instruction receives the block with its inner terminators
/// but without the terminator of the last selected line, which is preserved
/// outside. Indices inside the instruction are relative to the block, and the
/// result may contain any number of lines (`uniqln`, `dle`, … may change the
/// line count). Errors match `dlni`/`slnc`: zero, missing or reversed lines and
/// empty documents.
///
/// DSL: `onlni {start} {end} apply {instruction};`
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, instructions::onlni::Onlni, transforms::uniqln::Uniqln};
///
/// let inner = (Box::new(Uniqln::default()) as Box<dyn InstructionMethods>).into();
/// let token = Onlni::new(1, 3, inner);
/// assert_eq!(token.transform("b\nb\na\nc".into(), None).unwrap(), "b\na\nc");
/// ```
#[derive(Clone, Default)]
pub struct Onlni {
    start_line: usize,
    end_line: usize,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl Onlni {
    pub fn new(start_line: usize, end_line: usize, inner: TokenWrapper) -> Self {
        Onlni {
            start_line,
            end_line,
            params: vec![
                start_line.into(),
                end_line.into(),
                TextForgeParamTypes::Token(inner.clone()),
            ],
            inner,
        }
    }
}

impl InstructionMethods for Onlni {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x54
    }
    fn get_string_repr(&self) -> &'static str {
        "onlni"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!(
            "onlni {} {} apply {}",
            self.start_line,
            self.end_line,
            self.inner.to_textforge_line()
        ))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let range = line_section(&input, self.start_line, self.end_line, "onlni")?;
        match context {
            Some(context) => within(
                context,
                |input| {
                    format!(
                        "onlni {} {}  (block: {} chars)",
                        self.start_line,
                        self.end_line,
                        input[range.clone()].chars().count()
                    )
                },
                "onlni",
                input,
                |input, context| apply_to_section(input, range.clone(), &self.inner, context),
            ),
            None => apply_to_section(
                input,
                range,
                &self.inner,
                &mut GlobalExecutionContext::new(),
            ),
        }
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 3, "onlni", "param parsing error, invalid vec len")?;
        self.start_line = parse_args!(params, 0, Usize, "First argument should be a line number");
        self.end_line = parse_args!(params, 1, Usize, "Second argument should be a line number");
        self.inner = parse_args!(params, 2, Token, "Third argument should be an instruction");
        self.params = vec![
            self.start_line.into(),
            self.end_line.into(),
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
                TextForgeParamTypes::Usize(self.start_line),
                TextForgeParamTypes::Usize(self.end_line),
                TextForgeParamTypes::Token(self.inner.clone()),
            ]
        ))
    }
}
