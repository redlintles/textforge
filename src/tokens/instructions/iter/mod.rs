use std::borrow::Cow;

use crate::{
    context::{
        debug_trace::within,
        execution_context::{GlobalContextMethods, GlobalExecutionContext, VarEntry, VarValues},
    },
    parse_args,
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

#[cfg(feature = "test_access")]
pub mod test;

/// Iter - Iterate
///
/// Applies `inner` `times` times. During iteration `i` (starting at zero) the
/// body can read the index as `__COUNTER__` inside `[[ ]]` expressions. Nested
/// iterations hide the enclosing counter and restore it when they finish.
/// `end` in the body stops the remaining iterations.
///
/// ```rust
/// use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// plan.add(InstructionSpec::new("iter".into(), vec![
///     Argument::Number(3),
///     Argument::Instruction(Box::new(InstructionSpec::new("ate".into(), vec![
///         Argument::Text("[[__COUNTER__]]".into()),
///     ]))),
/// ])).unwrap();
/// assert_eq!(plan.compile().unwrap().process("n").unwrap(), "n012");
/// ```
#[derive(Clone, Default)]
pub struct Iter {
    times: usize,
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

/// Name of the iteration index variable visible to the body.
const COUNTER: &str = "__COUNTER__";

impl Iter {
    /// Run the body `times` times; iteration `i` observes `__COUNTER__ == i`.
    fn run<'a>(
        &self,
        input: Cow<'a, str>,
        context: &mut GlobalExecutionContext,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut result = input;
        for i in 0..self.times {
            // `end` in the body stops the remaining iterations too.
            if context.is_halted {
                break;
            }
            context.get_mut_var(COUNTER)?.value = VarValues::Usize(i);
            result = self.inner.apply_nested(result, context)?;
        }
        Ok(result)
    }
}

impl InstructionMethods for Iter {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x3c
    }
    fn get_string_repr(&self) -> &'static str {
        "iter"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        Cow::from(format!(
            "iter {} times {}",
            self.times,
            self.inner.to_textforge_line()
        ))
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let context = context.ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::RequiredContextError(
                    "Context required for iter instruction".into(),
                ),
                "iter",
                "",
            )
        })?;

        // A nested iteration hides the enclosing counter and restores it on
        // exit, so every level observes its own iteration index.
        let enclosing = context.take_var(COUNTER);
        context.add_var(
            COUNTER,
            VarEntry {
                mutable: true,
                value: VarValues::Usize(0),
            },
        )?;
        let result = within(
            context,
            |_| format!("iter {} times", self.times),
            "iter",
            input,
            |input, context| self.run(input, context),
        );
        context.rm_var(COUNTER)?;
        if let Some(entry) = enclosing {
            context.add_var(COUNTER, entry)?;
        }
        result
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 2, "iter", "param parsing error, invalid vec len")?;
        self.times = parse_args!(params, 0, Usize, "First argument should be of type usize");
        self.inner = parse_args!(
            params,
            1,
            Token,
            "Second argument should be of type instruction"
        );
        self.params = vec![self.times.into(), self.inner.clone().into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.times),
                TextForgeParamTypes::Token(self.inner.clone()),
            ]
        );
        Ok(result)
    }
}
