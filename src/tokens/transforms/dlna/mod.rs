use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        lines::{check_line, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// `dlna {line};` — Delete the lines after `line`.
///
/// Line indices start at 0 (Golden Rule 22); shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::dlna::Dlna};
/// let token = Dlna::new(0).unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "a\n");
/// ```
#[derive(Clone)]
pub struct Dlna {
    line: usize,
    params: Vec<TextForgeParamTypes>,
}
impl Dlna {
    /// Construct `dlna`. Line indices start at 0.
    pub fn new(line: usize) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[line.into()])?;
        Ok(token)
    }
}
impl Default for Dlna {
    fn default() -> Self {
        Self {
            line: 0,
            params: vec![0usize.into()],
        }
    }
}
impl InstructionMethods for Dlna {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "dlna"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        let mut parts = vec![self.get_string_repr().to_owned()];
        for parameter in &self.params {
            parts.push(match parameter {
                TextForgeParamTypes::String(value) => {
                    crate::text::line_parser::quote(value).into_owned()
                }
                TextForgeParamTypes::Usize(value) => value.to_string(),
                _ => String::new(),
            });
        }
        format!("{};\n", parts.join(" ")).into()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 1, "dlna", "")?;
        let line = crate::parse_args!(params, 0, Usize, "line");
        self.line = line;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_line(self.line, lines(&input).count(), "dlna", &input)?;
        let length: usize = lines(&input)
            .take(self.line + 1)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        Ok(crate::utils::allocation::keep(input, 0..length))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x44
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;
