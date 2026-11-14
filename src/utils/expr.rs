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
/// - A `{{`, `}}`, `[[` or `]]` that does not close a valid marker (a name
///   starting with a digit, a missing end…) is removed, unless escaped. To
///   print `{{name}}` literally, escape both ends: `\{{name\}}`; escaping one
///   end leaves the other unmatched, and it is removed.
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
    // Fast path: nenhum dos quatro delimitadores aparece de forma nenhuma
    // (completa, solta ou escapada) — nada a fazer.
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
            // esc.as_str() é "\{{", "\}}", "\[[" ou "\]]" — descarta o \.
            out.push_str(&esc.as_str()[1..]);
        } else if let Some(var_name) = caps.name("var") {
            let value: TextForgeParamTypes = reader.value(var_name.as_str())?.into_owned().into();
            let as_string: String = value.try_into()?;
            out.push_str(&as_string);
        } else if let Some(expr_src) = caps.name("expr") {
            let result = evaluate_with(expr_src.as_str(), reader, "api::expr")?;
            out.push_str(&value_to_plain_string(&result));
        }
        // Nenhum grupo nomeado bateu: caiu no fallback de delimitador solto
        // (`{{`, `}}`, `[[` ou `]]` incompleto/inválido) — não empurra nada
        // pra `out`, ou seja, remove esse trecho.

        last_end = m.end();
    }

    out.push_str(&input[last_end..]);
    Ok(out)
}
/// Evaluates an expression against the current variables and returns the
/// result as plain text (strings without the quotes evalexpr's `Display` adds).
/// `eval` and `[[ ]]` interpolation use the same evaluation.
pub fn expr(expression: &str, context: &GlobalExecutionContext) -> Result<String, TextForgeError> {
    let result = evaluate(expression, context, "api::expr")?;
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
            TextForgeErrorCode::InvalidExprError(Cow::from(e.to_string())),
            Cow::from(stage),
            Cow::from(expression.to_string()),
        )
    };
    let tree = evalexpr::build_operator_tree::<DefaultNumericTypes>(expression).map_err(invalid)?;
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
            TextForgeErrorCode::InvalidExprError(Cow::from(e.to_string())),
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
