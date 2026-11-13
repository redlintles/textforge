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

/// Onln - On Line
///
/// Applies one instruction to line `line` (base 1) only. The instruction
/// receives the line content without its terminator, so every index it uses is
/// relative to that line; the terminator stays outside and is preserved. The
/// result may contain any number of lines. `onln n` is equivalent to
/// `onlni n n`. Line zero, a missing line or an empty document is an error.
///
/// DSL: `onln {line} apply {instruction};`
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, instructions::onln::Onln, transforms::tbs::Tbs};
///
/// let token = Onln::new(2, (Box::new(Tbs::default()) as Box<dyn InstructionMethods>).into());
/// assert_eq!(token.transform("  a  \n  b  \n".into(), None).unwrap(), "  a  \nb\n");
/// ```
#[derive(Clone, Default)]
pub struct Onln {
    line: usize,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl Onln {
    pub fn new(line: usize, inner: TokenWrapper) -> Self {
        Onln {
            line,
            params: vec![line.into(), TextForgeParamTypes::Token(inner.clone())],
            inner,
        }
    }
}

impl InstructionMethods for Onln {
    /// Holds only immutable bound state, so it can be bound once; the nested
    /// instruction keeps its own (possibly dynamic) parameters.
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x52
    }
    fn get_string_repr(&self) -> &'static str {
        "onln"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!(
            "onln {} apply {}",
            self.line,
            self.inner.to_textforge_line()
        ))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let range = line_section(&input, self.line, self.line, "onln")?;
        match context {
            Some(context) => within(
                context,
                |input| {
                    format!(
                        "onln {}  (line: {} chars)",
                        self.line,
                        input[range.clone()].chars().count()
                    )
                },
                "onln",
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
        check_vec_len(params, 2, "onln", "param parsing error, invalid vec len")?;
        self.line = parse_args!(params, 0, Usize, "First argument should be a line number");
        self.inner = parse_args!(params, 1, Token, "Second argument should be an instruction");
        self.params = vec![
            self.line.into(),
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
                TextForgeParamTypes::Usize(self.line),
                TextForgeParamTypes::Token(self.inner.clone()),
            ]
        ))
    }
}
