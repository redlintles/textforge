#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use regex::Regex;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

use crate::parser::params::TextForgeParamTypes;
/// `rlw {pattern} {text};` — Replace the last match.
///
/// Replace the last occurrence of `pattern` in `input` with `text_to_replace`.
/// The text is unchanged when nothing matches. As in `raw`, the replacement is
/// a template: `$1` and `${name}` insert capture groups, `$$` is a literal `$`.
///
/// See Also:
///
/// - [`RAW` - Replace All With](crate::tokens::transforms::raw)
/// - [`RCW` - Replace Count With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace First With](crate::tokens::transforms::rfw)
/// - [`RNW` - Replace Nth With](crate::tokens::transforms::rnw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rlw::Rlw};
///
/// let token = Rlw::new(&"a", "b").unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(),None).unwrap().to_string(), "aaaab");
///
/// let groups = Rlw::new(&"(\\w)(\\d)", "$2$1").unwrap();
/// assert_eq!(groups.transform("a1 b2".into(),None).unwrap().to_string(), "a1 2b");
/// ```
///
#[derive(Clone, Debug)]
pub struct Rlw {
    /// The regular expression to match.
    pub pattern: Regex,
    /// The replacement text: a regex replacement template, where `$1` and
    /// `${name}` insert capture groups and `$$` is a literal `$` (so a price
    /// is written `$$5.00`; `$5.00` would insert group 5).
    pub text_to_replace: String,
    params: Vec<TextForgeParamTypes>,
}

impl Rlw {
    /// Builds `rlw {pattern} {text};` with the given parameters.
    ///
    /// Fails with the regex error message if `pattern` is not a valid regular expression.
    pub fn new(pattern: &str, text_to_replace: &str) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rlw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                pattern.to_string().into(),
                text_to_replace.to_string().into(),
            ],
            pattern,
        })
    }
}

impl Default for Rlw {
    fn default() -> Self {
        Rlw {
            pattern: crate::utils::regexes::EMPTY_RE.clone(),
            text_to_replace: "".to_string(),
            params: vec!["".to_string().into(), "".to_string().into()],
        }
    }
}

impl InstructionMethods for Rlw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("rlw {} {};\n", self.pattern, self.text_to_replace).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        // No match: the text is returned as it is.
        let Some(captures) = self.pattern.captures_iter(input.as_ref()).last() else {
            return Ok(input);
        };
        // The replacement is a template, as in `raw`, `rfw` and `rcw`: `$1`
        // inserts a capture group and `$$` a literal `$`. Up to 0.3.1 it was
        // inserted as written.
        let range = captures.get(0).map_or(0..0, |m| m.range());
        let mut replacement = String::new();
        captures.expand(&self.text_to_replace, &mut replacement);
        drop(captures);
        Ok(crate::utils::allocation::replace(
            input,
            range,
            &replacement,
        ))
    }

    fn get_string_repr(&self) -> &'static str {
        "rlw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 2, "rlw", "")?;

        let pattern_payload = parse_args!(params, 0, String, "pattern");

        self.pattern = Regex::new(&pattern_payload)
            .map_err(|error| crate::utils::errors::regex_error("rlw", &pattern_payload, error))?;

        self.text_to_replace = parse_args!(params, 1, String, "replacement");

        self.params = vec![
            self.pattern.to_string().into(),
            self.text_to_replace.to_string().into(),
        ];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1e
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.text_to_replace.clone()),
            ]
        );
        Ok(result)
    }
}
