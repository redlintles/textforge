use super::builder::tree::Node;
use crate::{
    api::TextForgeBuilderMethods, parser::params::TextForgeParamTypes,
    parser::resolve_var::TokenWrapper, tokens::InstructionMethods, utils::errors::TextForgeError,
};

/// Wraps each child separately; the condition is re-evaluated for every child.
/// This is not a single condition guarding an entire body.
///
/// ```compile_fail
/// use textforge::api::{conditional_builder::ConditionalBuilderEach, TextForgeBlockMethods};
/// fn forbidden(body: &mut ConditionalBuilderEach) {
///     body.block_assoc("inner", |_| Ok(())).unwrap();
/// }
/// ```
pub struct ConditionalBuilderEach {
    token: Box<dyn InstructionMethods>,
    params: Vec<TextForgeParamTypes>,
    conditional_tokens: Vec<Box<dyn InstructionMethods>>,
    children: Vec<Node>,
}

impl ConditionalBuilderEach {
    pub fn new(token: Box<dyn InstructionMethods>, params: Vec<TextForgeParamTypes>) -> Self {
        ConditionalBuilderEach {
            token,
            params,
            conditional_tokens: Vec::new(),
            children: Vec::new(),
        }
    }

    pub(crate) fn into_parts(self) -> (Node, Vec<Box<dyn InstructionMethods>>) {
        (
            Node::Group {
                token: self.token,
                params: self.params,
                children: self.children,
            },
            self.conditional_tokens,
        )
    }

    pub fn build(self) -> Vec<Box<dyn InstructionMethods>> {
        self.conditional_tokens
    }
}

// push_token funciona normalmente para incrementar conditional_tokens
impl TextForgeBuilderMethods for ConditionalBuilderEach {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError> {
        let child = t.into();
        let mut new_token: Box<dyn InstructionMethods> = self.token.clone();

        let mut param_vec = self.params.clone();

        param_vec.push(child.clone().into());

        new_token.set_params(&param_vec)?;

        self.conditional_tokens.push(new_token);
        self.children.push(Node::Instruction(child));

        Ok(())
    }
}
