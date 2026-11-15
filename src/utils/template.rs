//! Parameter templates compiled once, when a pipeline is registered.
//!
//! [`interpolate_with`](super::expr::interpolate_with) scans its input with
//! `INTERP_RE` and compiles every `[[ ]]` expression on each call. A
//! registered pipeline runs the same parameter text many times, so
//! [`Template::parse`] does that work once: it splits the text into literal
//! pieces, variable reads and expressions whose operator trees are already
//! built. [`Template::render`] then produces exactly what `interpolate_with`
//! would, with the same errors, in the same order.
use std::borrow::Cow;

use evalexpr::{DefaultNumericTypes, HashMapContext, Node, Value};

use crate::{
    context::builtins::VarReader,
    parser::params::TextForgeParamTypes,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        expr::{evaluate_tree, is_literal_brackets, value_to_plain_string},
    },
};

/// Stage label of interpolation errors (as in `interpolate_with`).
const STAGE: &str = "api::expr";

/// One `[[ ]]` expression with its operator tree built in advance.
#[derive(Debug)]
pub(crate) struct CompiledExpr {
    source: Box<str>,
    tree: Result<Node<DefaultNumericTypes>, String>,
    /// Result of an expression that reads no variable and calls no function:
    /// it cannot change between runs, so it is evaluated once.
    constant: Option<Result<Value, String>>,
}

impl CompiledExpr {
    pub(crate) fn new(source: &str) -> Self {
        let tree = evalexpr::build_operator_tree::<DefaultNumericTypes>(source)
            .map_err(|error| error.to_string());
        let constant = tree.as_ref().ok().and_then(|tree| {
            let pure = tree.iter_read_variable_identifiers().next().is_none()
                && tree.iter_function_identifiers().next().is_none();
            pure.then(|| {
                tree.eval_with_context(&HashMapContext::<DefaultNumericTypes>::new())
                    .map_err(|error| error.to_string())
            })
        });
        CompiledExpr {
            source: source.into(),
            tree,
            constant,
        }
    }

    fn error(&self, message: &str, stage: &'static str) -> TextForgeError {
        TextForgeError::new(
            TextForgeErrorCode::InvalidExprError(Cow::from(message.to_owned())),
            Cow::from(stage),
            Cow::from(self.source.to_string()),
        )
    }

    /// Same result and errors as `evaluate_with(source, reader, stage)`.
    pub(crate) fn evaluate(
        &self,
        reader: &VarReader,
        stage: &'static str,
    ) -> Result<Value, TextForgeError> {
        if let Some(constant) = &self.constant {
            return constant
                .clone()
                .map_err(|message| self.error(&message, stage));
        }
        let tree = self
            .tree
            .as_ref()
            .map_err(|message| self.error(message, stage))?;
        evaluate_tree(tree, &self.source, reader, stage)
    }
}

#[derive(Debug)]
enum Segment {
    Text(Box<str>),
    Var(Box<str>),
    Expr(CompiledExpr),
}

/// A parameter text split into literal text, `{{var}}` reads and `[[expr]]`
/// expressions (escapes resolved; stray delimiters are literal text).
#[derive(Debug)]
pub(crate) struct Template {
    segments: Vec<Segment>,
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

const PAIRS: [&[u8]; 4] = [b"{{", b"}}", b"[[", b"]]"];

impl Template {
    /// Split `input` the way `INTERP_RE` does. `None` when the text holds no
    /// delimiter at all, so the caller keeps using the text as it is.
    ///
    /// At each position the first matching alternative wins, as in the regex:
    /// an escaped pair, `{{name}}`, `[[expr]]` (shortest), then a stray pair,
    /// which stays in the text.
    pub(crate) fn parse(input: &str) -> Option<Self> {
        if !PAIRS
            .iter()
            .any(|pair| input.contains(std::str::from_utf8(pair).unwrap_or_default()))
        {
            return None;
        }
        let bytes = input.as_bytes();
        let mut segments = Vec::new();
        let mut text = String::new();
        let mut start = 0;
        let mut i = 0;
        while i < bytes.len() {
            let rest = &bytes[i..];
            let pair = |at: usize| PAIRS.iter().any(|pair| rest[at..].starts_with(pair));
            // Literal text up to `i`, then whatever the match produced.
            let flush = |text: &mut String, start: usize| text.push_str(&input[start..i]);
            if rest[0] == b'\\' && pair(1) {
                flush(&mut text, start);
                text.push_str(&input[i + 1..i + 3]);
                i += 3;
                start = i;
                continue;
            }
            if rest.starts_with(b"{{") {
                let name_len = rest[2..]
                    .iter()
                    .take_while(|byte| is_name_byte(**byte))
                    .count();
                if name_len > 0 && is_name_start(rest[2]) && rest[2 + name_len..].starts_with(b"}}")
                {
                    flush(&mut text, start);
                    if !text.is_empty() {
                        segments.push(Segment::Text(std::mem::take(&mut text).into()));
                    }
                    segments.push(Segment::Var(input[i + 2..i + 2 + name_len].into()));
                    i += 4 + name_len;
                    start = i;
                } else {
                    // Not a variable read: the pair is ordinary text.
                    i += 2;
                }
                continue;
            }
            if rest.starts_with(b"[[") {
                let close = rest[2..].windows(2).position(|window| window == b"]]");
                match close {
                    Some(end) if is_literal_brackets(&input[i + 2..i + 2 + end]) => {
                        // Empty or a POSIX class: ordinary text, both ends.
                        i += 4 + end;
                    }
                    Some(end) => {
                        flush(&mut text, start);
                        if !text.is_empty() {
                            segments.push(Segment::Text(std::mem::take(&mut text).into()));
                        }
                        segments.push(Segment::Expr(CompiledExpr::new(&input[i + 2..i + 2 + end])));
                        i += 4 + end;
                        start = i;
                    }
                    None => i += 2,
                }
                continue;
            }
            if rest.starts_with(b"}}") || rest.starts_with(b"]]") {
                // A closing pair that closes nothing is ordinary text.
                i += 2;
                continue;
            }
            i += 1;
        }
        text.push_str(&input[start..]);
        if !text.is_empty() {
            segments.push(Segment::Text(text.into()));
        }
        Some(Template { segments })
    }

    /// Whether rendering reads nothing that can change between runs: only
    /// literal text and constant expressions.
    pub(crate) fn is_constant(&self) -> bool {
        self.segments.iter().all(|segment| match segment {
            Segment::Text(_) => true,
            Segment::Var(_) => false,
            Segment::Expr(expression) => expression.constant.is_some(),
        })
    }

    /// Same output and errors as `interpolate_with(source, reader)`.
    pub(crate) fn render(&self, reader: &VarReader) -> Result<String, TextForgeError> {
        let mut out = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Text(text) => out.push_str(text),
                Segment::Var(name) => {
                    let value: TextForgeParamTypes = reader.value(name)?.into_owned().into();
                    let as_string: String = value.try_into()?;
                    out.push_str(&as_string);
                }
                Segment::Expr(expression) => {
                    out.push_str(&value_to_plain_string(&expression.evaluate(reader, STAGE)?));
                }
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
