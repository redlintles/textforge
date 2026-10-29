use super::{builder::tree::Node, conditional_builder::ConditionalBuilderEach};
use crate::tokens::instructions::ifdc::Ifdc;
use crate::{
    api::{TextForgeBuilderMethods, TextForgeConditionalMethods},
    parser::params::TextForgeParamTypes,
    parser::resolve_var::TokenWrapper,
    tokens::{InstructionMethods, instructions::blk::Blk},
    utils::errors::TextForgeError,
};

/// Collects a block body and lowers each child to one `blk` instruction.
/// Nested block declarations are forbidden, including beneath conditional tokens.
///
/// ```compile_fail
/// use textforge::api::{block_builder::BlockBuilder, TextForgeBlockMethods};
/// let mut body = BlockBuilder::new("outer");
/// body.block_assoc("inner", |_| Ok(())).unwrap();
/// ```
pub struct BlockBuilder {
    block_name: &'static str,
    block_tokens: Vec<Box<dyn InstructionMethods>>,
    children: Vec<Node>,
}

impl BlockBuilder {
    pub fn new(block_name: &'static str) -> Self {
        BlockBuilder {
            block_name,
            block_tokens: Vec::new(),
            children: Vec::new(),
        }
    }

    pub(crate) fn into_node(self) -> Node {
        Node::Group {
            token: Box::new(Blk::default()),
            params: vec![TextForgeParamTypes::String(self.block_name.to_string())],
            children: self.children,
        }
    }

    pub fn build(self) -> Vec<Box<dyn InstructionMethods>> {
        self.block_tokens
    }
}

impl TextForgeBuilderMethods for BlockBuilder {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError> {
        let token = t.into();
        crate::parser::structure::validate_block_child(&token)?;
        let param_vec: Vec<TextForgeParamTypes> =
            vec![self.block_name.to_string().into(), token.clone().into()];

        let mut new_block = Box::new(Blk::default());
        new_block.set_params(&param_vec)?;

        self.block_tokens.push(new_block);
        self.children.push(Node::Instruction(token));
        Ok(())
    }
}

impl TextForgeConditionalMethods for BlockBuilder {
    fn if_do_contains_each<F>(&mut self, value: &str, f: F) -> Result<&mut Self, TextForgeError>
    where
        F: FnOnce(&mut ConditionalBuilderEach) -> Result<(), TextForgeError>,
    {
        let mut child = ConditionalBuilderEach::new(
            Box::new(Ifdc::default()),
            vec![TextForgeParamTypes::String(value.to_string())],
        );
        f(&mut child)?;
        let (node, legacy) = child.into_parts();
        // Keep the existing infallible standalone build() contract. Stage all
        // legacy wrappers locally so a failed group never partially mutates us.
        let mut staged = BlockBuilder::new(self.block_name);
        for token in legacy {
            staged.push_token(token)?;
        }
        self.block_tokens.extend(staged.block_tokens);
        self.children.push(node);
        Ok(self)
    }
}
