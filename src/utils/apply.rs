use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    parser::resolve_var::TokenWrapper,
    utils::errors::{ErrorManager, TextForgeError},
};

pub fn apply_transform<'a>(
    token: &TokenWrapper,
    input: Cow<'a, str>,
    error_manager: &mut ErrorManager,
    context: &mut GlobalExecutionContext,
) -> Result<Cow<'a, str>, TextForgeError> {
    match token.apply_token(input, &mut *context) {
        Ok(x) => Ok(x),
        Err(e) => {
            error_manager.add_error(e.clone());
            Err(e)
        }
    }
}

/// The single execution loop shared by every processing mode.
///
/// `step` applies one token to the current document and returns the next one;
/// each mode (plain, debug, watchers, buffered) supplies its own `step`, so the
/// document type `D` can be a `Cow<str>`, a buffered document or a shared
/// snapshot. The loop owns the rules every mode must agree on: tokens run in
/// order, and execution stops before the next token once `end` halts the
/// context.
pub(crate) fn run_tokens<D>(
    tokens: &[TokenWrapper],
    mut document: D,
    context: &mut GlobalExecutionContext,
    mut step: impl FnMut(
        usize,
        &TokenWrapper,
        D,
        &mut GlobalExecutionContext,
    ) -> Result<D, TextForgeError>,
) -> Result<D, TextForgeError> {
    for (index, token) in tokens.iter().enumerate() {
        if context.is_halted {
            break;
        }
        document = step(index, token, document, context)?;
    }
    Ok(document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::reader::read_from_text;

    #[test]
    fn run_tokens_applies_in_order_and_stops_after_end() {
        let tokens: Vec<_> = ["atb A;", "ate B;", "end;", "ate C;"]
            .iter()
            .map(|line| read_from_text(line).unwrap())
            .collect();
        let mut seen = Vec::new();
        let mut context = GlobalExecutionContext::new();
        let output = run_tokens(
            &tokens,
            Cow::Borrowed("x"),
            &mut context,
            |index, token, document, context| {
                seen.push(index);
                token.apply_token(document, context)
            },
        )
        .unwrap();
        assert_eq!(output, "AxB");
        assert_eq!(seen, vec![0, 1, 2]);
    }

    #[test]
    fn run_tokens_propagates_the_first_error() {
        let tokens: Vec<_> = ["dlc 5 9;", "ate B;"]
            .iter()
            .map(|line| read_from_text(line).unwrap())
            .collect();
        let mut context = GlobalExecutionContext::new();
        let result = run_tokens(
            &tokens,
            Cow::Borrowed("x"),
            &mut context,
            |_, token, document, context| token.apply_token(document, context),
        );
        assert!(result.is_err());
    }
}
