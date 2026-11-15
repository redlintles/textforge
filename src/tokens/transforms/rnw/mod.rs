#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext, parse_args,
    utils::validations::check_vec_len,
};

use regex::Regex;

use crate::{tokens::InstructionMethods, utils::errors::TextForgeError};

use crate::parser::params::TextForgeParamTypes;
/// `rnw {pattern} {text} {n};` — Replace the `n`th match.
///
/// Replace the `n`th occurrence (base 0) of `pattern` in `input` with
/// `text_to_replace`. The text is unchanged when there is no match `n`. As in
/// `raw`, the replacement is a template: `$1` and `${name}` insert capture
/// groups, `$$` is a literal `$`.
///
/// See Also:
///
/// - [`RAW` - Replace All With](crate::tokens::transforms::raw)
/// - [`RCW` - Replace Count With](crate::tokens::transforms::rcw)
/// - [`RFW` - Replace First With](crate::tokens::transforms::rfw)
/// - [`RLW` - Replace Last With](crate::tokens::transforms::rlw)
///
/// # Example:
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rnw::Rnw};
///
/// let token = Rnw::new(&"a", "b", 2).unwrap();
///
/// assert_eq!(token.transform("aaaaa".into(),None).unwrap().to_string(), "aabaa");
///
/// let groups = Rnw::new(&"(\\w)(\\d)", "$2$1", 1).unwrap();
/// assert_eq!(groups.transform("a1 b2 c3".into(),None).unwrap().to_string(), "a1 2b c3");
/// ```
///
#[derive(Clone, Debug)]
pub struct Rnw {
    /// The regular expression to match.
    pub pattern: Regex,
    /// The replacement text: a regex replacement template, where `$1` and
    /// `${name}` insert capture groups and `$$` is a literal `$` (so a price
    /// is written `$$5.00`; `$5.00` would insert group 5).
    pub text_to_replace: String,
    /// Which match to replace (the first is 1).
    pub index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Rnw {
    /// Builds `rnw {pattern} {text} {n};` with the given parameters.
    ///
    /// Fails with the regex error message if `pattern` is not a valid regular expression.
    pub fn new(pattern: &str, text_to_replace: &str, index: usize) -> Result<Self, String> {
        let pattern = Regex::new(pattern).map_err(|x| x.to_string())?;
        Ok(Rnw {
            text_to_replace: text_to_replace.to_string(),
            params: vec![
                text_to_replace.to_string().into(),
                pattern.to_string().into(),
                index.into(),
            ],
            pattern,
            index,
        })
    }
}

impl Default for Rnw {
    fn default() -> Self {
        Rnw {
            pattern: crate::utils::regexes::EMPTY_RE.clone(),
            text_to_replace: "".to_string(),
            index: 0,
            params: vec!["".to_string().into(), "".to_string().into(), (0).into()],
        }
    }
}

impl InstructionMethods for Rnw {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!(
            "rnw {} {} {};\n",
            self.pattern, self.text_to_replace, self.index
        )
        .into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        // No match number `n`: the text is returned as it is.
        let Some(captures) = self.pattern.captures_iter(input.as_ref()).nth(self.index) else {
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
        "rnw"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 3, "rnw", "")?;

        let pattern_payload = parse_args!(params, 0, String, "pattern");

        self.pattern = Regex::new(&pattern_payload)
            .map_err(|error| crate::utils::errors::regex_error("rnw", &pattern_payload, error))?;

        self.text_to_replace = parse_args!(params, 1, String, "replacement");

        self.index = parse_args!(params, 2, Usize, "index");

        self.params = vec![
            self.pattern.to_string().into(),
            self.text_to_replace.to_string().into(),
            self.index.into(),
        ];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x1f
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.pattern.to_string()),
                TextForgeParamTypes::String(self.text_to_replace.clone()),
                TextForgeParamTypes::Usize(self.index),
            ]
        );
        Ok(result)
    }
}
