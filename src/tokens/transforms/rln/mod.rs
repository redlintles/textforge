use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::TextForgeError,
        lines::{check_line, invalid, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// `rln {line} {text};` — Replace the content of `line`, keeping its terminator.
///
/// Line indices start at 0 (Golden Rule 22); shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::rln::Rln};
/// let token = Rln::new(1, "x").unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "a\nx");
/// ```
#[derive(Clone)]
pub struct Rln {
    line: usize,
    text: String,
    params: Vec<TextForgeParamTypes>,
}
impl Rln {
    /// Construct `rln`. Line indices start at 0.
    pub fn new(line: usize, text: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[line.into(), text.to_owned().into()])?;
        Ok(token)
    }
}
impl Default for Rln {
    fn default() -> Self {
        Self {
            line: 0,
            text: String::new(),
            params: vec![0usize.into(), String::new().into()],
        }
    }
}
impl InstructionMethods for Rln {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "rln"
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
        check_vec_len(params, 2, "rln", "")?;
        let line = crate::parse_args!(params, 0, Usize, "line");
        let text = crate::parse_args!(params, 1, String, "text");
        if text.chars().any(crate::utils::lines::is_break) {
            return Err(invalid("rln", "Expected a single line", &text));
        }
        self.line = line;
        self.text = text;

        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_line(self.line, lines(&input).count(), "rln", &input)?;
        let start: usize = lines(&input)
            .take(self.line)
            .map(|(t, e)| t.len() + e.len())
            .sum();
        let (text, ending) = lines(&input).nth(self.line).unwrap_or(("", ""));
        let end = start + text.len();
        if self.text.is_empty() && ending.is_empty() {
            let ending = lines(&input)
                .find_map(|(_, e)| (!e.is_empty()).then_some(e))
                .unwrap_or("\n");
            let replacement = ending.to_owned();
            return Ok(crate::utils::allocation::replace(
                input,
                start..end,
                &replacement,
            ));
        }
        Ok(crate::utils::allocation::replace(
            input,
            start..end,
            &self.text,
        ))
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x4a
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;
