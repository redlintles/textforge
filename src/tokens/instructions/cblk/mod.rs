use std::borrow::Cow;

use crate::{
    context::{
        debug_trace::{plural, within},
        execution_context::{GlobalContextMethods, GlobalExecutionContext},
    },
    parse_args,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{apply::run_tokens, errors::TextForgeError, validations::check_vec_len},
};

#[cfg(feature = "test_access")]
pub mod test;
/// `cblk {name};` — Run the instructions of block `name`.
#[derive(Clone)]
pub struct Cblk {
    block_name: String,
    params: Vec<TextForgeParamTypes>,
}

impl Default for Cblk {
    fn default() -> Self {
        Cblk {
            block_name: "x".to_string(),
            params: vec!["x".to_string().into()],
        }
    }
}

impl InstructionMethods for Cblk {
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x35
    }
    // Bound state is immutable; only the execution context changes.
    fn supports_preparation(&self) -> bool {
        true
    }
    fn get_string_repr(&self) -> &'static str {
        "cblk"
    }

    fn to_textforge_line(&self) -> std::borrow::Cow<'static, str> {
        format!("cblk {};\n", self.block_name).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let context = context.ok_or_else(|| {
            crate::utils::errors::context_required("cblk", "find the block it runs")
        })?;
        let tokens = context.take_block(&self.block_name)?;
        let result = within(
            context,
            |_| {
                format!(
                    "cblk {}  ({} {})",
                    self.block_name,
                    tokens.len(),
                    plural(tokens.len(), "instruction")
                )
            },
            &self.block_name,
            input,
            |result, context| {
                // Same halting rule as the root: nothing runs after `end`.
                run_tokens(&tokens, result, context, |_, token, result, context| {
                    token.apply_nested(result, context)
                })
            },
        )?;

        // On error the block stays taken, as before (see the cblk tests).
        context.put_block(&self.block_name, tokens);
        Ok(result)
    }

    fn set_params(
        &mut self,
        params: &[crate::parser::params::TextForgeParamTypes],
    ) -> Result<(), crate::utils::errors::TextForgeError> {
        check_vec_len(params, 1, "cblk", "")?;

        self.block_name = parse_args!(params, 0, String, "block name");
        self.params = vec![self.block_name.to_string().into()];
        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result = to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::String(self.block_name.to_string()),]
        );

        Ok(result)
    }
}
