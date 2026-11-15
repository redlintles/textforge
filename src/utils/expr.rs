use std::{borrow::Cow, collections::HashMap};

use evalexpr::{
    Context, ContextWithMutableVariables, DefaultNumericTypes, EvalexprResult, HashMapContext,
    Value,
};

use crate::{
    context::{
        builtins::{CurrentText, VarReader},
        execution_context::{GlobalExecutionContext, VarEntry, VarValues},
        static_context::GlobalFunctions,
    },
    parser::params::TextForgeParamTypes,
    utils::{
        errors::{
            TextForgeError,
            TextForgeErrorCode::{self, InvalidExprError},
        },
        regexes::INTERP_RE,
    },
};

/// Builds a snapshot evalexpr context from the current variable table, so
/// expressions can reference stored variables by name.
///
/// Numeric conversion is a resolution-time concern only: everything in
/// `GlobalExecutionContext` stays `VarValues::String`/`Usize` before and
/// after `eval` runs — this function never mutates the context, it only
/// reads from it to build a disposable `HashMapContext`. Each variable is
/// tried as an `i64`; if it doesn't parse, it's passed to evalexpr as a
/// plain string. Floats aren't attempted on purpose: ATP's smallest
/// logical unit is a character, always represented as an integer, so
/// float support isn't worth the extra ambiguity.
///
/// Token variables are skipped: they aren't representable as evalexpr
/// values. Referencing one in an expression will surface as evalexpr's own
/// "identifier not found" error, which is an acceptable failure mode here.
pub fn build_eval_context(
    vars: &HashMap<String, VarEntry>,
) -> Result<HashMapContext, TextForgeError> {
    let mut ctx = HashMapContext::new();
    for (name, entry) in vars {
        set_eval_value(&mut ctx, name, &entry.value)?;
    }
    Ok(ctx)
}

/// Add one variable to an evalexpr context (token variables are skipped).
fn set_eval_value(
    ctx: &mut HashMapContext,
    name: &str,
    value: &VarValues,
) -> Result<(), TextForgeError> {
    let Some(value) = to_eval_value(value) else {
        return Ok(());
    };
    ctx.set_value(name.to_owned(), value).map_err(|e| {
        TextForgeError::new(
            InvalidExprError(Cow::from(e.to_string())),
            Cow::from("eval.build_eval_context"),
            Cow::from(name.to_owned()),
        )
    })
}

/// A variable as an evalexpr value: integers when the text parses as one,
/// `None` for token variables.
fn to_eval_value(value: &VarValues) -> Option<Value> {
    Some(match value {
        VarValues::Usize(n) => Value::from_int(*n as i64),
        VarValues::String(s) => match s.parse::<i64>() {
            Ok(i) => Value::from_int(i),
            Err(_) => Value::from(s.clone()),
        },
        VarValues::Token(_) => return None,
    })
}

/// The variables one expression reads, looked up by a short linear scan
/// instead of a hashed map: expressions read a handful of names.
struct ReadContext<'f> {
    values: Vec<(&'f str, Value)>,
    functions: &'f GlobalFunctions,
}

impl Context for ReadContext<'_> {
    type NumericTypes = DefaultNumericTypes;

    fn get_value(&self, identifier: &str) -> Option<&Value> {
        self.values
            .iter()
            .find(|(name, _)| *name == identifier)
            .map(|(_, value)| value)
    }

    fn call_function(&self, identifier: &str, argument: &Value) -> EvalexprResult<Value> {
        self.functions
            .call(identifier, argument)
            .unwrap_or_else(|| {
                Err(evalexpr::EvalexprError::FunctionIdentifierNotFound(
                    identifier.to_string(),
                ))
            })
    }

    fn are_builtin_functions_disabled(&self) -> bool {
        false
    }

    fn set_builtin_functions_disabled(&mut self, disabled: bool) -> EvalexprResult<()> {
        if disabled {
            Err(evalexpr::EvalexprError::BuiltinFunctionsCannotBeDisabled)
        } else {
            Ok(())
        }
    }
}

/// Whether a parameter in a numeric position is an interpolated expression
/// (`[[__COUNTER__ + 1]]`, `{{a}}{{b}}`) rather than a literal number. Such
/// parameters are kept as text when parsed and become a number only after
/// interpolation at execution time.
pub(crate) fn is_numeric_template(text: &str) -> bool {
    text.contains("[[") || text.contains("{{")
}

/// Whether the content between `[[` and `]]` is plain text rather than an
/// expression: empty, or starting with `:` as POSIX character classes do
/// (`[[:alpha:]]`, `[[:alpha:][:digit:]]`). No expression starts with a colon.
pub(crate) fn is_literal_brackets(content: &str) -> bool {
    content.trim().is_empty() || content.starts_with(':')
}

/// Turn `\n`, `\t`, `\r` and `\0` inside the double-quoted texts of an
/// expression into the characters they name, as in double-quoted pipeline
/// parameters. evalexpr itself accepts only `\"` and `\\`, which are kept
/// for it, so `[["a\nb"]]` was an "illegal escape" error up to 0.3.1.
/// Outside quotes nothing changes.
pub(crate) fn expand_string_escapes(expression: &str) -> Cow<'_, str> {
    if !expression.contains('\\') {
        return Cow::Borrowed(expression);
    }
    let mut out = String::with_capacity(expression.len());
    let mut in_string = false;
    let mut chars = expression.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                in_string = !in_string;
                out.push(c);
            }
            '\\' if in_string => match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            },
            _ => out.push(c),
        }
    }
    Cow::Owned(out)
}

/// How a value is named in error messages.
fn describe_value(value: &Value) -> String {
    match value {
        Value::String(text) => format!("the text {text:?}"),
        Value::Int(number) => format!("the integer {number}"),
        Value::Float(number) => format!("the decimal number {number}"),
        Value::Boolean(flag) => format!("the boolean {flag}"),
        Value::Tuple(_) => "a tuple".into(),
        Value::Empty => "nothing".into(),
    }
}

fn describe_type(kind: &evalexpr::ValueType) -> &'static str {
    match kind {
        evalexpr::ValueType::String => "text",
        evalexpr::ValueType::Int => "an integer",
        evalexpr::ValueType::Float => "a decimal number",
        evalexpr::ValueType::Boolean => "a boolean",
        evalexpr::ValueType::Tuple => "a tuple",
        evalexpr::ValueType::Empty => "nothing",
    }
}

/// Note added where a type mismatch may come from a variable that is text.
const TYPES_NOTE: &str = "TextForge values are integers and text; a variable whose value is not an integer (`2.5`, `true`) is text";

/// An evalexpr error in TextForge's terms. Up to 0.3.1 evalexpr's own message
/// was shown ("The operator Add was called with a wrong combination of types:
/// [String, Int]").
pub(crate) fn describe_eval_error(error: &evalexpr::EvalexprError) -> String {
    use evalexpr::EvalexprError as E;
    match error {
        E::WrongTypeCombination { operator, actual } => {
            let types: Vec<_> = actual.iter().map(describe_type).collect();
            format!(
                "`{}` cannot be applied to {}; {TYPES_NOTE}",
                operator.to_string().trim(),
                types.join(" and ")
            )
        }
        E::ExpectedInt { actual } | E::ExpectedNumber { actual } => {
            format!(
                "an integer is needed here, but this is {}; {TYPES_NOTE}",
                describe_value(actual)
            )
        }
        E::ExpectedString { actual } => {
            format!(
                "text is needed here, but this is {}",
                describe_value(actual)
            )
        }
        E::ExpectedNumberOrString { actual } => format!(
            "an integer or text is needed here, but this is {}",
            describe_value(actual)
        ),
        E::ExpectedBoolean { actual } => format!(
            "a condition (such as `a > 1`) is needed here, but this is {}; {TYPES_NOTE}",
            describe_value(actual)
        ),
        E::VariableIdentifierNotFound(name) => {
            format!(
                "the variable `{name}` does not exist: define it with `var` or `val` before using it"
            )
        }
        E::FunctionIdentifierNotFound(name) => format!("there is no function named `{name}`"),
        E::UnmatchedLBrace => "a `(` is never closed".into(),
        E::UnmatchedRBrace => "a `)` has no matching `(`".into(),
        E::UnmatchedDoubleQuote => {
            "a text in double quotes is never closed: a `\"` is missing".into()
        }
        E::DivisionError { dividend, divisor } | E::ModulationError { dividend, divisor } => {
            format!(
                "cannot divide {} by {}",
                describe_value(dividend),
                describe_value(divisor)
            )
        }
        E::AdditionError { .. }
        | E::SubtractionError { .. }
        | E::MultiplicationError { .. }
        | E::NegationError { .. } => "the result is too large for an integer".into(),
        E::IllegalEscapeSequence(sequence) => format!(
            "`{sequence}` is not a valid escape in a quoted text inside an expression (valid: `\\n`, `\\t`, `\\r`, `\\0`, `\\\"`, `\\\\`)"
        ),
        other => other.to_string(),
    }
}

/// Converts an evalexpr result back into a plain string, without the
/// formatting evalexpr's own `Display` impl adds (e.g. wrapping strings in
/// literal quotes, since that impl is meant to print back valid expression
/// syntax, not a clean value).
pub fn value_to_plain_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Tuple(t) => {
            let parts: Vec<String> = t.iter().map(value_to_plain_string).collect();
            format!("({})", parts.join(", "))
        }
        Value::Empty => String::new(),
    }
}

/// Expands the `{{var}}` and `[[expr]]` markers in `input`.
///
/// - `{{name}}` becomes the value of the variable `name`.
/// - `[[expr]]` becomes the result of the expression.
/// - A delimiter preceded by `\` (`\{{`, `\}}`, `\[[`, `\]]`) is printed
///   literally and the `\` is dropped. Elsewhere (`\n`, a lone `\`, `\{`) a
///   backslash has no special meaning.
/// - A `{{`, `}}`, `[[` or `]]` that does not form a valid marker (a name
///   that is not an identifier, a missing end…) is kept as written, so JSON
///   (`{"a":{"b":1}}`), nested brackets (`m[i[0]]`) and regex classes
///   (`[0-9[:alpha:]]`) pass through unchanged. Before 0.3.2 such delimiters
///   were removed.
/// - `[[ ]]` whose content is empty or starts with `:` is not an expression
///   and is kept as written: no expression starts with a colon, and POSIX
///   character classes (`[[:alpha:]]`) do.
/// - To print a valid marker literally, escape its opening: `\{{name}}`.
/// - Nothing is scanned twice: a value that happens to contain `{{` or `[[` is
///   not interpolated again, so variables that refer to each other cannot loop.
///
/// Built-in variables that describe the current text (`__LINES__`,
/// `__LEN_CHARS__`) need a running instruction and fail here.
pub fn interpolate(
    input: &str,
    context: &GlobalExecutionContext,
) -> Result<String, TextForgeError> {
    interpolate_with(input, &VarReader::new(context, CurrentText::Unavailable))
}

/// [`interpolate`] reading variables through `reader`, which also knows the
/// text the running instruction receives.
pub(crate) fn interpolate_with(input: &str, reader: &VarReader) -> Result<String, TextForgeError> {
    // Fast path: none of the four delimiters appears at all
    // (complete, stray or escaped), so there is nothing to do.
    if !input.contains("{{")
        && !input.contains("}}")
        && !input.contains("[[")
        && !input.contains("]]")
    {
        return Ok(input.to_string());
    }

    let mut out = String::with_capacity(input.len());
    let mut last_end = 0;

    for caps in INTERP_RE.captures_iter(input) {
        let m = caps.get(0).unwrap();
        out.push_str(&input[last_end..m.start()]);

        if let Some(esc) = caps.name("esc") {
            // esc.as_str() is "\{{", "\}}", "\[[" or "\]]"; drop the \.
            out.push_str(&esc.as_str()[1..]);
        } else if let Some(var_name) = caps.name("var") {
            let value: TextForgeParamTypes = reader.value(var_name.as_str())?.into_owned().into();
            let as_string: String = value.try_into()?;
            out.push_str(&as_string);
        } else if let Some(expr_src) = caps.name("expr") {
            if is_literal_brackets(expr_src.as_str()) {
                out.push_str(m.as_str());
            } else {
                let result = evaluate_with(expr_src.as_str(), reader, "")?;
                out.push_str(&value_to_plain_string(&result));
            }
        } else {
            // No named group matched: a `{{`, `}}`, `[[` or `]]` that is not
            // part of a valid marker. It is ordinary text.
            out.push_str(m.as_str());
        }

        last_end = m.end();
    }

    out.push_str(&input[last_end..]);
    Ok(out)
}
/// Evaluates an expression against the current variables and returns the
/// result as plain text (strings without the quotes evalexpr's `Display` adds).
/// `eval` and `[[ ]]` interpolation use the same evaluation.
pub fn expr(expression: &str, context: &GlobalExecutionContext) -> Result<String, TextForgeError> {
    let result = evaluate(expression, context, "")?;
    Ok(value_to_plain_string(&result))
}

/// Compile and evaluate `expression` against the variables it reads.
///
/// Same steps and errors as `evalexpr::eval_with_context` (compile, then
/// evaluate), but the evaluation context holds only the variables the
/// expression reads, so its cost no longer grows with every variable stored in
/// the pipeline (see `docs/audits/0.3.0-on-debug-audit.md`, item 3).
pub(crate) fn evaluate(
    expression: &str,
    context: &GlobalExecutionContext,
    stage: &'static str,
) -> Result<Value, TextForgeError> {
    evaluate_with(
        expression,
        &VarReader::new(context, CurrentText::Unavailable),
        stage,
    )
}

/// [`evaluate`] reading variables (and built-ins) through `reader`. An unknown
/// identifier is left out, so evalexpr reports it as its own error.
pub(crate) fn evaluate_with(
    expression: &str,
    reader: &VarReader,
    stage: &'static str,
) -> Result<Value, TextForgeError> {
    let invalid = |e: evalexpr::EvalexprError| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidExprError(Cow::from(describe_eval_error(&e))),
            Cow::from(stage),
            Cow::from(expression.to_string()),
        )
    };
    let tree =
        evalexpr::build_operator_tree::<DefaultNumericTypes>(&expand_string_escapes(expression))
            .map_err(invalid)?;
    evaluate_tree(&tree, expression, reader, stage)
}

/// Evaluate an already built operator tree; errors carry `expression`.
pub(crate) fn evaluate_tree(
    tree: &evalexpr::Node<DefaultNumericTypes>,
    expression: &str,
    reader: &VarReader,
    stage: &'static str,
) -> Result<Value, TextForgeError> {
    let invalid = |e: evalexpr::EvalexprError| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidExprError(Cow::from(describe_eval_error(&e))),
            Cow::from(stage),
            Cow::from(expression.to_string()),
        )
    };
    let mut eval_ctx = ReadContext {
        values: Vec::new(),
        functions: reader.context().static_context().functions(),
    };
    for name in tree.iter_read_variable_identifiers() {
        if eval_ctx.get_value(name).is_some() {
            continue;
        }
        match reader.value(name) {
            Ok(value) => {
                if let Some(value) = to_eval_value(&value) {
                    eval_ctx.values.push((name, value));
                }
            }
            Err(error) if matches!(error.error_code, TextForgeErrorCode::VariableNotFound(_)) => {}
            Err(error) => return Err(error),
        }
    }
    tree.eval_with_context(&eval_ctx).map_err(invalid)
}
