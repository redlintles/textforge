use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        lines::{invalid, lines},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// `inl {line} {text};` — Insert a line before `line` (one past the last appends).
///
/// Line indices start at 0 (Golden Rule 22); shared logical-line semantics. See `docs/lines.md`.
///
/// ```
/// use textforge::tokens::{InstructionMethods, transforms::inl::Inl};
/// let token = Inl::new(1, "x").unwrap();
/// assert_eq!(token.transform("a\nb".into(), None).unwrap(), "a\nx\nb");
/// ```
#[derive(Clone)]
pub struct Inl {
    line: usize,
    text: String,
    params: Vec<TextForgeParamTypes>,
}
impl Inl {
    /// Construct `inl`. Line indices start at 0.
    pub fn new(line: usize, text: &str) -> Result<Self, TextForgeError> {
        let mut token = Self::default();
        token.set_params(&[line.into(), text.to_owned().into()])?;
        Ok(token)
    }
}
impl Default for Inl {
    fn default() -> Self {
        Self {
            line: 0,
            text: String::new(),
            params: vec![0usize.into(), String::new().into()],
        }
    }
}
impl InstructionMethods for Inl {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "inl"
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
        check_vec_len(params, 2, "inl", "")?;
        let line = crate::parse_args!(params, 0, Usize, "line");
        let text = crate::parse_args!(params, 1, String, "text");
        if text.chars().any(crate::utils::lines::is_break) {
            return Err(invalid("inl", "Expected a single line", &text));
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
        let count = lines(&input).count();
        // Any line may be inserted before, and the line count appends.
        if self.line > count {
            return Err(TextForgeError::new(
                TextForgeErrorCode::IndexOutOfRange(
                    format!(
                        "cannot insert before line {}: the text has {} (insert before 0 to {count}; {count} appends)",
                        self.line,
                        crate::utils::errors::plural(count, "line")
                    )
                    .into(),
                ),
                "inl",
                crate::utils::errors::input_excerpt(&input),
            ));
        }
        Ok(crate::utils::allocation::insert_line(
            input, self.line, &self.text,
        ))
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x48
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;
