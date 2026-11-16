use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        lines::{check_range, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// `slnc {start} {end};` — Keep only lines `start..=end`.
///
/// Line indices start at 0 (Golden Rule 22); shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::slnc::Slnc};
/// let token = Slnc::new(1, 1).unwrap();
/// assert_eq!(token.transform("a\nb\nc".into(), None).unwrap(), "b\n");
/// ```
#[derive(Clone)]
pub struct Slnc {
    start_line: usize,
    end_line: usize,
    params: Vec<TextForgeParamTypes>,
}
impl Slnc {
    /// Construct `slnc`. Line indices start at 0.
    pub fn new(start_line: usize, end_line: usize) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[start_line.into(), end_line.into()])?;
        Ok(token)
    }
}
impl Default for Slnc {
    fn default() -> Self {
        Self {
            start_line: 0,
            end_line: 0,
            params: vec![0usize.into(), 0usize.into()],
        }
    }
}
impl InstructionMethods for Slnc {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "slnc"
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
        check_vec_len(params, 2, "slnc", "")?;
        let start_line = crate::parse_args!(params, 0, Usize, "start");
        let end_line = crate::parse_args!(params, 1, Usize, "end");
        self.start_line = start_line;
        self.end_line = end_line;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_range(
            self.start_line,
            self.end_line,
            lines(&input).count(),
            "slnc",
            &input,
        )?;
        let start: usize = lines(&input)
            .take(self.start_line)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        let length: usize = lines(&input)
            .skip(self.start_line)
            .take((self.end_line - self.start_line).saturating_add(1))
            .map(|(t, e)| t.len() + e.len())
            .sum();
        Ok(crate::utils::allocation::keep(input, start..start + length))
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x46
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;
