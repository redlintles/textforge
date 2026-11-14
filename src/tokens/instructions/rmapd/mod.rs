use crate::{
    context::execution_context::GlobalExecutionContext,
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};
use std::borrow::Cow;

/// `rmapd {map} add {pattern} {text};` — Add a rule to replacement map `map`.
///
/// ```
/// use textforge::tokens::instructions::rmapd::Rmapd;
/// let _token = Rmapd::default();
/// ```
#[derive(Clone, Default)]
pub struct Rmapd {
    params: Vec<TextForgeParamTypes>,
    #[allow(clippy::type_complexity)]
    // Shared, bounded per-template cache; no global mutable map state.
    cache: std::sync::Arc<
        std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<regex::Regex>>>,
    >,
    compiled: Option<std::sync::Arc<regex::Regex>>,
}
impl InstructionMethods for Rmapd {
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn get_string_repr(&self) -> &'static str {
        "rmapd"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        let values: Vec<_> = self
            .params
            .iter()
            .filter_map(|p| match p {
                TextForgeParamTypes::String(s) => Some(shell_words::quote(s).into_owned()),
                _ => None,
            })
            .collect();
        if values.len() == 3 {
            format!("rmapd {} add {} {};\n", values[0], values[1], values[2]).into()
        } else {
            "rmapd x add x x;\n".into()
        }
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 3, "rmapd", "")?;
        let _name = parse_args!(params, 0, String, "map name");
        let pattern = parse_args!(params, 1, String, "pattern");
        let _replacement = parse_args!(params, 2, String, "replacement");
        let mut cache = self.cache.lock().map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::GenericError(
                "internal error: the regular expression cache is unusable because another thread panicked while using it".into(),
            ),
                "rmapd",
                "cache",
            )
        })?;
        let regex = if let Some(regex) = cache.get(&pattern) {
            regex.clone()
        } else {
            let regex =
                std::sync::Arc::new(regex::Regex::new(&pattern).map_err(|error| {
                    crate::utils::errors::regex_error("rmapd", &pattern, error)
                })?);
            // Bound memory for interpolated patterns; constants retain their prepared regex.
            if cache.len() >= 64 {
                cache.clear();
            }
            cache.insert(pattern, regex.clone());
            regex
        };
        self.compiled = Some(regex);
        self.params = params.to_vec();
        Ok(())
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        check_vec_len(&self.params, 3, "rmapd", "")?;
        let context = context.ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::RequiredContextError(
                "`rmapd` needs an execution context to store the replacement map; run it in a pipeline (TextForgeProcessor) instead of calling transform() without a context".into(),
            ),
                "rmapd",
                "",
            )
        })?;
        let name = match &self.params[0] {
            TextForgeParamTypes::String(name) => name.as_str(),
            _ => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::InvalidParameters(
                        "the replacement map name must be text".into(),
                    ),
                    "",
                    "",
                ));
            }
        };
        let replacement = parse_args!(&self.params, 2, String, "replacement");
        let regex = self.compiled.as_ref().ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::InvalidParameters(
                    "the replacement rule has no pattern yet: its parameters were never set".into(),
                ),
                "rmapd",
                name.to_owned(),
            )
        })?;
        context.define_replace_rule(name, regex.clone(), replacement);
        Ok(input)
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3f
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        Ok(crate::to_bytecode!(self.get_opcode(), self.params))
    }
}
#[cfg(test)]
mod test;
