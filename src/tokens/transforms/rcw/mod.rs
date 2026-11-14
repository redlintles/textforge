#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use regex::Regex;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;
use crate::tokens::InstructionMethods;
use crate::utils::validations::check_vec_len;

/// `rcw {pattern} {text} {count};` — Replace the first `count` matches.
///
/// Replace `count` ocurrences of `pattern` in `input` with `text_to_replace`
///
/// See Also:
///
/// - [`RAW` - Replace All With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace First With](crate::tokens::transforms::rfw)
/// - [`RLW` - Replace Last With](crate::tokens::transforms::rlw)
/// - [`RNW` - Replace Nth With](crate::tokens::transforms::rnw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rcw::Rcw};
///
/// let token = Rcw::new(&"a", "b", 3).unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(), None).unwrap().to_string(), "bbbaa");
/// ```
///
#[derive(Clone, Debug)]
pub struct Rcw {
    /// The regular expression to match.
    pub pattern: Regex,
    /// How many matches to replace.
    pub count: usize,
    /// The replacement text (`$1` refers to a capture group).
    pub text_to_replace: String,
    params: Vec<TextForgeParamTypes>,
}

impl Rcw {
    /// Builds `rcw {pattern} {text} {count};` with the given parameters.
    ///
    /// Fails with the regex error message if `pattern` is not a valid regular expression.
    pub fn new(pattern: &str, text_to_replace: &str, count: usize) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rcw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                pattern.to_string().into(),
                text_to_replace.to_string().into(),
                count.into(),
            ],
            pattern,
            count,
        })
    }
}

impl Default for Rcw {
    fn default() -> Self {
        Rcw {
            pattern: crate::utils::regexes::EMPTY_RE.clone(),
            text_to_replace: "".to_string(),
            count: 0_usize,
            params: vec!["".to_string().into(), "".to_string().into(), (0).into()],
        }
    }
}

impl InstructionMethods for Rcw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!(
            "rcw {} {} {};\n",
            self.pattern, self.text_to_replace, self.count
        )
        .into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if self.count == 0 {
            return Ok(input);
        }
        match input {
            Cow::Borrowed(v) => {
                Ok(self
                    .pattern
                    .replacen(v.as_ref(), self.count, &self.text_to_replace))
            }

            Cow::Owned(v) => {
                match self
                    .pattern
                    .replacen(v.as_ref(), self.count, &self.text_to_replace)
                {
                    Cow::Borrowed(_) => Ok(Cow::Owned(v)),

                    Cow::Owned(result) => Ok(Cow::Owned(result)),
                }
            }
        }
    }

    fn get_string_repr(&self) -> &'static str {
        "rcw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 3, "rcw", "")?;

        let pattern_payload = parse_args!(params, 0, String, "pattern");

        self.pattern = Regex::new(&pattern_payload)
            .map_err(|error| crate::utils::errors::regex_error("rcw", &pattern_payload, error))?;

        self.text_to_replace = parse_args!(params, 1, String, "replacement");

        self.count = parse_args!(params, 2, Usize, "count");
        self.params = vec![
            self.pattern.to_string().into(),
            self.text_to_replace.to_string().into(),
            self.count.into(),
        ];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x10
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.text_to_replace.clone()),
                TextForgeParamTypes::Usize(self.count),
            ]
        );
        Ok(result)
    }
}
