//! Temporary construction hierarchy. Only lowered instruction vectors escape to
//! the processor; neither the parser nor the execution engine stores this tree.
use crate::{
    parser::{
        params::TextForgeParamTypes,
        resolve_var::{TokenWrapper, ValType},
        structure,
    },
    tokens::{InstructionMethods, instructions::null::Null},
    utils::errors::TextForgeError,
};

/// A construction leaf or a group that wraps each child independently.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBlockMethods,
///     TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().block_assoc("example", |body| {
///     body.trim_both_sides()?;
///     Ok(())
/// }).unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
pub(crate) enum Node {
    Instruction(TokenWrapper),
    Group {
        token: Box<dyn InstructionMethods>,
        params: Vec<TextForgeParamTypes>,
        children: Vec<Node>,
    },
}

/// Validate the entire hierarchy before cloning leaves or creating output.
pub(crate) fn validate_and_lower(nodes: &[Node]) -> Result<Vec<TokenWrapper>, TextForgeError> {
    let mut pending: Vec<_> = nodes.iter().map(|node| (node, 0, false)).collect();
    while let Some((node, depth, inside_block)) = pending.pop() {
        match node {
            Node::Instruction(token) => structure::validate_at(token, depth, inside_block)?,
            Node::Group {
                token,
                params,
                children,
            } => {
                // Check the group's own signature independently of its children,
                // including empty groups, without executing or resolving values.
                let mut header = params
                    .iter()
                    .cloned()
                    .map(ValType::Literal)
                    .collect::<Vec<_>>();
                header.push(ValType::Literal(TextForgeParamTypes::Token(
                    TokenWrapper::new(Box::new(Null::default()), None),
                )));
                structure::validate_at(
                    &TokenWrapper::new(token.clone(), Some(header)),
                    depth,
                    inside_block,
                )?;
                let inside_block = inside_block || token.get_string_repr() == "blk";
                pending.extend(
                    children
                        .iter()
                        .map(|child| (child, depth + 1, inside_block)),
                );
            }
        }
    }

    // Ancestors are bounded by validation. Process children from left to right,
    // wrapping from the innermost group out to preserve declaration semantics.
    let mut output = Vec::new();
    let mut pending: Vec<(&Node, Vec<&Node>)> =
        nodes.iter().rev().map(|node| (node, Vec::new())).collect();
    while let Some((node, ancestors)) = pending.pop() {
        match node {
            Node::Group { children, .. } => {
                let mut ancestors = ancestors;
                ancestors.push(node);
                pending.extend(
                    children
                        .iter()
                        .rev()
                        .map(|child| (child, ancestors.clone())),
                );
            }
            Node::Instruction(token) => {
                let mut token = token.clone();
                for ancestor in ancestors.iter().rev() {
                    if let Node::Group {
                        token: template,
                        params,
                        ..
                    } = ancestor
                    {
                        let mut params = params
                            .iter()
                            .cloned()
                            .map(ValType::Literal)
                            .collect::<Vec<_>>();
                        params.push(ValType::Literal(token.into()));
                        token = TokenWrapper::new(template.clone(), Some(params));
                    }
                }
                output.push(token);
            }
        }
    }
    Ok(output)
}
