use std::{borrow::Cow, ops::Deref, sync::Arc};

use crate::{
    context::{
        builtins::{CurrentText, VarReader},
        execution_context::GlobalExecutionContext,
    },
    globals::table::{QuerySource, QueryTarget, SyntaxDef, SyntaxToken, TOKEN_TABLE, TargetValue},
    parser::params::{MAX_DEPTH_ASSOC_PAYLOAD, TextForgeParamTypes},
    tokens::{InstructionMethods, instructions::null::Null},
    utils::{
        errors::{ErrorParameter, TextForgeError, TextForgeErrorCode},
        expr::{interpolate_with, is_numeric_template},
        regexes::VAR_REF_RE,
        template::Template,
    },
};

#[derive(Clone, Debug)]
pub enum ValType {
    Literal(TextForgeParamTypes),
    VarRef(String),
}

impl From<String> for ValType {
    fn from(value: String) -> Self {
        ValType::from(value.as_str())
    }
}

impl From<&str> for ValType {
    fn from(value: &str) -> Self {
        match VAR_REF_RE.captures(value) {
            Some(caps) => Self::VarRef(caps[1].to_string()),
            None => Self::Literal(TextForgeParamTypes::String(value.to_string())),
        }
    }
}
impl From<usize> for ValType {
    fn from(value: usize) -> Self {
        Self::Literal(TextForgeParamTypes::Usize(value))
    }
}

impl From<ValType> for TextForgeParamTypes {
    fn from(value: ValType) -> Self {
        match value {
            ValType::Literal(v) => v,
            ValType::VarRef(name) => TextForgeParamTypes::VarRef(name),
        }
    }
}

pub struct TokenWrapper {
    params: Vec<ValType>,
    pub token: Box<dyn InstructionMethods>,
    prepared: Option<Arc<dyn InstructionMethods>>,
    compiled: Option<Arc<CompiledParams>>,
    memo: Option<Arc<BindingMemo>>,
    /// The file and line the instruction was read from, for error traces.
    source: Option<Arc<SourceLine>>,
}

/// Where an instruction was written: `file:line`.
#[derive(Debug)]
struct SourceLine {
    file: String,
    line: usize,
}

/// The instruction that failed, as it was bound: its line with the values it
/// ran with, and its parameters. Built only on the error path.
pub(crate) struct Bound {
    line: String,
    parameters: Vec<ErrorParameter>,
}

/// Longest parameter value an error report shows.
const MAX_PARAMETER_CHARS: usize = 60;

fn describe_value(value: &TextForgeParamTypes) -> Option<String> {
    let text = match value {
        TextForgeParamTypes::String(text) => {
            let shown: String = text.chars().take(MAX_PARAMETER_CHARS).collect();
            let more = text.chars().count().saturating_sub(MAX_PARAMETER_CHARS);
            let suffix = if more > 0 {
                format!("… +{more} characters")
            } else {
                String::new()
            };
            format!("\"{}\"{suffix}", shown.escape_debug())
        }
        TextForgeParamTypes::Usize(number) => number.to_string(),
        TextForgeParamTypes::Token(token) => token
            .to_textforge_line()
            .trim_end_matches(['\n', ';'])
            .to_owned(),
        TextForgeParamTypes::VarRef(name) => format!("{{{{{name}}}}}"),
        _ => return None,
    };
    Some(text)
}

/// The last binding of an instruction whose parameters are dynamic, kept for
/// instructions with immutable bound state: when the next run resolves the
/// same text and numbers (a variable set once, a counter cycling through a few
/// values), the bound instruction (a compiled regex, say) is reused instead of
/// being rebuilt. Concurrent runs that find it busy simply bind afresh.
#[derive(Default)]
struct BindingMemo {
    last: std::sync::Mutex<Option<Binding>>,
}

/// Resolved parameters and the instruction bound with them.
type Binding = (Vec<TextForgeParamTypes>, Arc<dyn InstructionMethods>);

impl std::fmt::Debug for BindingMemo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BindingMemo")
    }
}

/// Parameters a memo can compare: text and numbers only.
fn memo_key_matches(stored: &[TextForgeParamTypes], params: &[TextForgeParamTypes]) -> bool {
    stored.len() == params.len()
        && stored.iter().zip(params).all(|pair| match pair {
            (TextForgeParamTypes::String(a), TextForgeParamTypes::String(b)) => a == b,
            (TextForgeParamTypes::Usize(a), TextForgeParamTypes::Usize(b)) => a == b,
            _ => false,
        })
}

fn memoizable(params: &[TextForgeParamTypes]) -> bool {
    params.iter().all(|parameter| {
        matches!(
            parameter,
            TextForgeParamTypes::String(_) | TextForgeParamTypes::Usize(_)
        )
    })
}

/// What parameter binding needs that does not change between runs: the
/// expected parameter kinds and each string parameter's compiled template.
/// Built with the prepared binding, for instructions that cannot be bound once.
#[derive(Debug)]
struct CompiledParams {
    expected: Vec<SyntaxToken>,
    /// One entry per parameter; `Some` for a string holding delimiters.
    templates: Vec<Option<Template>>,
}

impl CompiledParams {
    /// `None` when the parameters do not fit the instruction's syntax, so
    /// binding keeps reporting that error the usual way.
    fn new(token: &dyn InstructionMethods, params: &[ValType]) -> Option<Self> {
        let expected = match TOKEN_TABLE.find((
            QuerySource::Identifier(token.get_string_repr().into()),
            QueryTarget::Syntax,
        )) {
            Ok(TargetValue::Syntax(syntax)) => get_effective_param_types(&syntax),
            _ => return None,
        };
        if expected.len() != params.len() {
            return None;
        }
        let templates = params
            .iter()
            .map(|parameter| match parameter {
                ValType::Literal(TextForgeParamTypes::String(text)) => Template::parse(text),
                _ => None,
            })
            .collect();
        Some(CompiledParams {
            expected,
            templates,
        })
    }
}

/// Deepest nested level `prepare_constant` descends to; matches the parsers'
/// largest nesting limit, so every parseable child can be prepared.
const MAX_PREPARED_DEPTH: usize = MAX_DEPTH_ASSOC_PAYLOAD as usize;

// Internal clones (parameter resolution clones nested wrappers on every
// execution) share the prepared binding, so nested instructions are bound once.
// The only public exit, `get_transform_vec`, uses `without_preparation`, so an
// exported, mutable public template never carries a stale cache.
impl Clone for TokenWrapper {
    fn clone(&self) -> Self {
        Self {
            params: self.params.clone(),
            token: self.token.clone(),
            prepared: self.prepared.clone(),
            compiled: self.compiled.clone(),
            memo: self.memo.clone(),
            source: self.source.clone(),
        }
    }
}

impl Default for TokenWrapper {
    fn default() -> Self {
        TokenWrapper {
            params: Vec::new(),
            token: Box::new(Null::default()),
            prepared: None,
            compiled: None,
            memo: None,
            source: None,
        }
    }
}

impl Deref for TokenWrapper {
    type Target = Box<dyn InstructionMethods>;
    fn deref(&self) -> &Self::Target {
        &self.token
    }
}

impl From<Box<dyn InstructionMethods>> for TokenWrapper {
    fn from(value: Box<dyn InstructionMethods>) -> Self {
        let token_params = value
            .get_params()
            .clone()
            .into_iter()
            .map(ValType::Literal)
            .collect::<Vec<ValType>>();
        TokenWrapper {
            params: token_params,
            token: value,
            prepared: None,
            compiled: None,
            memo: None,
            source: None,
        }
    }
}

impl From<TokenWrapper> for Box<dyn InstructionMethods> {
    fn from(value: TokenWrapper) -> Self {
        value.token
    }
}

impl TokenWrapper {
    // Only called after ownership moves into a processor's private pipeline map.
    // Externally mutable public token templates never acquire a stale cache.
    pub(crate) fn prepare_constant(&mut self) {
        self.prepare_at(0);
    }

    /// Prepare nested instructions first, so the parameters bound for this
    /// instruction already carry their children's cached bindings.
    fn prepare_at(&mut self, depth: usize) {
        if depth < MAX_PREPARED_DEPTH {
            for parameter in &mut self.params {
                if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                    child.prepare_at(depth + 1);
                }
            }
        }
        // Plain literals need no compilation (the common case at load time).
        let dynamic = self.params.iter().any(|parameter| match parameter {
            ValType::Literal(TextForgeParamTypes::String(text)) => ["{{", "}}", "[[", "]]"]
                .iter()
                .any(|marker| text.contains(marker)),
            ValType::Literal(_) => false,
            ValType::VarRef(_) => true,
        });
        let compiled = dynamic
            .then(|| CompiledParams::new(self.token.as_ref(), &self.params))
            .flatten();
        self.bind_constant(compiled.as_ref());
        if self.prepared.is_none() {
            // Only text and number parameters can be compared between runs.
            let comparable = compiled.as_ref().is_some_and(|compiled| {
                compiled
                    .expected
                    .iter()
                    .all(|kind| matches!(kind, SyntaxToken::String | SyntaxToken::Usize))
            });
            if comparable && self.token.supports_preparation() {
                self.memo = Some(Arc::default());
            }
            self.compiled = compiled.map(Arc::new);
        }
    }

    /// Bind once when every parameter is constant: numbers, nested
    /// instructions, joined pipelines and text whose templates read nothing
    /// that changes between runs (constant `[[ ]]` expressions included).
    fn bind_constant(&mut self, compiled: Option<&CompiledParams>) {
        let constant_text = |index: usize, text: &str| match compiled {
            Some(compiled) => compiled.templates[index]
                .as_ref()
                .is_none_or(Template::is_constant),
            None => !["{{", "}}", "[[", "]]"]
                .iter()
                .any(|marker| text.contains(marker)),
        };
        if !self.token.supports_preparation()
            || !self
                .params
                .iter()
                .enumerate()
                .all(|(index, parameter)| match parameter {
                    ValType::Literal(TextForgeParamTypes::Usize(_))
                    | ValType::Literal(TextForgeParamTypes::Token(_))
                    | ValType::Literal(TextForgeParamTypes::Pipeline(_)) => true,
                    ValType::Literal(TextForgeParamTypes::String(text)) => {
                        constant_text(index, text)
                    }
                    _ => false,
                })
        {
            return;
        }
        let context = &mut GlobalExecutionContext::new();
        if let Ok(parameters) = ValType::resolve_variables(
            self.token.as_ref(),
            &self.params,
            compiled,
            context,
            CurrentText::Unavailable,
        ) {
            let mut token = self.token.clone();
            if token.set_params(&parameters).is_ok() {
                self.prepared = Some(Arc::from(token));
            }
        }
    }

    /// A copy with every cached binding removed, for wrappers that leave the
    /// processor as public, mutable templates.
    pub(crate) fn without_preparation(&self) -> Self {
        let mut copy = self.clone();
        copy.strip_preparation(0);
        copy
    }

    fn strip_preparation(&mut self, depth: usize) {
        self.prepared = None;
        self.compiled = None;
        self.memo = None;
        if depth < MAX_PREPARED_DEPTH {
            for parameter in &mut self.params {
                if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                    child.strip_preparation(depth + 1);
                }
            }
        }
    }

    pub fn get_params(&self) -> &Vec<ValType> {
        &self.params
    }
    /// Parameters, for loading joined pipelines in place (`parser::joins`).
    pub(crate) fn params_mut(&mut self) -> &mut [ValType] {
        &mut self.params
    }
    pub fn get_default_token(&self) -> Box<dyn InstructionMethods> {
        self.token.clone()
    }
    pub fn new(token: Box<dyn InstructionMethods>, params: Option<Vec<ValType>>) -> Self {
        match params {
            Some(param_vec) => TokenWrapper {
                params: param_vec,
                token,
                prepared: None,
                compiled: None,
                memo: None,
                source: None,
            },
            None => {
                let token_params = token
                    .get_params()
                    .clone()
                    .into_iter()
                    .map(ValType::Literal)
                    .collect::<Vec<ValType>>();
                TokenWrapper {
                    params: token_params,
                    token,
                    prepared: None,
                    compiled: None,
                    memo: None,
                    source: None,
                }
            }
        }
    }
    /// Bind once for debug execution and metadata, so logging cannot consult a
    /// token template whose default parameters differ from the executed values.
    /// `text` is what the instruction will receive (for `__LINES__` and similar).
    pub(crate) fn bind_for_debug(
        &self,
        context: &GlobalExecutionContext,
        text: CurrentText,
    ) -> Result<Box<dyn InstructionMethods>, TextForgeError> {
        let bind = || {
            let params = ValType::resolve_variables(
                self.token.as_ref(),
                &self.params,
                self.compiled.as_deref(),
                context,
                text,
            )?;
            let mut token = self.token.clone();
            token.set_params(&params)?;
            Ok(token)
        };
        bind().map_err(|error| self.locate(error))
    }

    /// Record the file and line this instruction was read from.
    /// Nested instructions (the body of `iter`, a block's instruction) share
    /// the line.
    pub(crate) fn set_source(&mut self, file: &str, line: usize) {
        let source = Arc::new(SourceLine {
            file: file.to_owned(),
            line,
        });
        let mut pending = vec![self];
        while let Some(wrapper) = pending.pop() {
            wrapper.source = Some(Arc::clone(&source));
            for parameter in &mut wrapper.params {
                if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                    pending.push(child);
                }
            }
        }
    }

    /// `file:line` of this instruction, when it was read from a file.
    pub(crate) fn source_location(&self) -> Option<String> {
        self.source
            .as_ref()
            .map(|source| format!("{}:{}", source.file, source.line))
    }

    fn unresolved_line(&self) -> String {
        match self.to_text_line_unresolved() {
            Ok(line) => line.trim_end().to_owned(),
            Err(_) => self.get_string_repr().to_owned(),
        }
    }

    /// Name this instruction in an error that does not say where it happened:
    /// its source line (`dls 9;`), or its mnemonic when it has no text form;
    /// and add its frame to the error's trace.
    pub(crate) fn locate(&self, error: TextForgeError) -> TextForgeError {
        self.locate_with(error, None)
    }

    /// [`Self::locate`] for an instruction that failed after binding
    /// `bound`: its frame shows the values it ran with, and its parameters.
    pub(crate) fn locate_bound(
        &self,
        error: TextForgeError,
        bound: &dyn InstructionMethods,
    ) -> TextForgeError {
        let bound = self.describe_bound(bound);
        self.locate_with(error, Some(Box::new(bound)))
    }

    fn locate_with(&self, mut error: TextForgeError, bound: Option<Box<Bound>>) -> TextForgeError {
        if error.instruction.is_empty() {
            error.instruction = self.unresolved_line().into();
        }
        // The instruction that failed shows the values it ran with; a
        // container around it shows its line as written.
        let failed_here = error.trace().next().is_none();
        let (line, parameters) = match bound {
            Some(bound) if failed_here => (bound.line, bound.parameters),
            _ => (self.unresolved_line(), Vec::new()),
        };
        error.push_frame(line, self.source_location(), || parameters);
        error
    }

    fn failed(
        &self,
        error: TextForgeError,
        bound: &dyn InstructionMethods,
    ) -> (TextForgeError, Option<Box<Bound>>) {
        (error, Some(Box::new(self.describe_bound(bound))))
    }

    /// The line and parameters of `bound`, this instruction bound to the
    /// values it ran with; the parameters as written come from `self`.
    pub(crate) fn describe_bound(&self, bound: &dyn InstructionMethods) -> Bound {
        let names = crate::globals::parameter_names::parameter_names(self.get_string_repr());
        let parameters = bound
            .get_params()
            .iter()
            .enumerate()
            .filter_map(|(index, value)| {
                let value = describe_value(value)?;
                let name = names.get(index).map_or_else(
                    || format!("parameter {}", index + 1),
                    |name| (*name).to_owned(),
                );
                let written = match self.params.get(index) {
                    Some(ValType::VarRef(variable)) => Some(format!("{{{{{variable}}}}}")),
                    Some(ValType::Literal(TextForgeParamTypes::String(text)))
                        if ["{{", "[["].iter().any(|marker| text.contains(marker)) =>
                    {
                        Some(format!("\"{}\"", text.escape_debug()))
                    }
                    _ => None,
                };
                Some(ErrorParameter::new(name, value, written))
            })
            .collect();
        Bound {
            line: bound.to_textforge_line().trim_end().to_owned(),
            parameters,
        }
    }

    pub fn apply_token<'a>(
        &self,
        input: Cow<'a, str>,
        context: &mut GlobalExecutionContext,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        self.apply_unlocated(input, context)
            .map_err(|(error, bound)| self.locate_with(error, bound))
    }

    /// Bind and run; a failure of the bound instruction carries what it was
    /// bound to (built on the error path only).
    fn apply_unlocated<'a>(
        &self,
        input: Cow<'a, str>,
        context: &mut GlobalExecutionContext,
    ) -> Result<Cow<'a, str>, (TextForgeError, Option<Box<Bound>>)> {
        if let Some(prepared) = &self.prepared {
            return prepared
                .transform(input, Some(context))
                .map_err(|error| self.failed(error, prepared.as_ref()));
        }
        let parsed_params = ValType::resolve_variables(
            self.token.as_ref(),
            &self.params,
            self.compiled.as_deref(),
            context,
            CurrentText::Str(&input),
        )
        .map_err(|error| (error, None))?;
        if let Some(memo) = &self.memo {
            let token = self
                .bind_memoized(memo, parsed_params)
                .map_err(|error| (error, None))?;
            return token
                .transform(input, Some(context))
                .map_err(|error| self.failed(error, token.as_ref()));
        }
        let mut t = self.token.clone();
        t.set_params(&parsed_params)
            .map_err(|error| (error, None))?;
        t.transform(input, Some(context))
            .map_err(|error| self.failed(error, t.as_ref()))
    }

    /// Bind `params`, reusing the memo's last binding when they are the same.
    fn bind_memoized(
        &self,
        memo: &BindingMemo,
        params: Vec<TextForgeParamTypes>,
    ) -> Result<Arc<dyn InstructionMethods>, TextForgeError> {
        if !memoizable(&params) {
            let mut token = self.token.clone();
            token.set_params(&params)?;
            return Ok(Arc::from(token));
        }
        if let Ok(last) = memo.last.try_lock()
            && let Some((stored, token)) = last.as_ref()
            && memo_key_matches(stored, &params)
        {
            return Ok(Arc::clone(token));
        }
        let mut token = self.token.clone();
        token.set_params(&params)?;
        let token: Arc<dyn InstructionMethods> = Arc::from(token);
        if let Ok(mut last) = memo.last.try_lock() {
            *last = Some((params, Arc::clone(&token)));
        }
        Ok(token)
    }

    /// Apply a token nested inside another instruction (`iter`, `cblk`, `on*`).
    /// Same result as [`Self::apply_token`]; it is the single place where nested
    /// steps are observed by debug tracing.
    pub(crate) fn apply_nested<'a>(
        &self,
        input: Cow<'a, str>,
        context: &mut GlobalExecutionContext,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let Some(trace) = context.trace_mut() else {
            return self.apply_token(input, context);
        };
        let mark = trace.mark();
        if !trace.will_show() {
            // Past the container's step limit: count it, measure nothing.
            let output = self.apply_token(input, context)?;
            if let Some(trace) = context.trace_mut() {
                trace.step(mark, None);
            }
            return Ok(output);
        }
        let before = input.chars().count();
        // Bind once so the reported line shows the values actually executed.
        let bound = self.bind_for_debug(context, CurrentText::Str(&input))?;
        let output = bound
            .transform(input, Some(context))
            .map_err(|error| self.locate_bound(error, bound.as_ref()))?;
        if let Some(trace) = context.trace_mut() {
            let line = bound.to_textforge_line();
            trace.step(mark, Some((&line, before, output.chars().count())));
        }
        Ok(output)
    }

    #[cfg(feature = "buffered")]
    pub(crate) fn apply_buffered(
        &self,
        input: crate::api::TextBuffer,
        context: &mut GlobalExecutionContext,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        self.apply_buffered_unlocated(input, context)
            .map_err(|(error, bound)| self.locate_with(error, bound))
    }

    #[cfg(feature = "buffered")]
    fn apply_buffered_unlocated(
        &self,
        mut input: crate::api::TextBuffer,
        context: &mut GlobalExecutionContext,
    ) -> Result<crate::api::TextBuffer, (TextForgeError, Option<Box<Bound>>)> {
        if let Some(prepared) = &self.prepared {
            return prepared
                .transform_buffered(input, Some(context))
                .map_err(|error| self.failed(error, prepared.as_ref()));
        }
        let params = ValType::resolve_variables(
            self.token.as_ref(),
            &self.params,
            self.compiled.as_deref(),
            context,
            CurrentText::Buffer(std::cell::RefCell::new(&mut input)),
        )
        .map_err(|error| (error, None))?;
        if let Some(memo) = &self.memo {
            let token = self
                .bind_memoized(memo, params)
                .map_err(|error| (error, None))?;
            return token
                .transform_buffered(input, Some(context))
                .map_err(|error| self.failed(error, token.as_ref()));
        }
        let mut token = self.token.clone();
        token.set_params(&params).map_err(|error| (error, None))?;
        token
            .transform_buffered(input, Some(context))
            .map_err(|error| self.failed(error, token.as_ref()))
    }

    pub fn to_text_line_resolved(
        &self,
        context: &mut GlobalExecutionContext,
    ) -> Result<String, TextForgeError> {
        let parsed_params = ValType::resolve_variables(
            self.token.as_ref(),
            &self.params,
            self.compiled.as_deref(),
            context,
            CurrentText::Unavailable,
        )?;
        let mut t = self.token.clone();
        t.set_params(&parsed_params)?;

        Ok(t.to_textforge_line().into())
    }

    pub fn to_text_line_unresolved(&self) -> Result<String, TextForgeError> {
        crate::parser::structure::validate(self)?;
        // Serialize owned descriptions using the same registry as the parser.
        // Individual token Display methods are intended for diagnostics and do
        // not consistently quote empty text, whitespace, quotes or backslashes.
        let mut pending = vec![(self, false)];
        let mut output = Vec::<String>::new();
        while let Some((wrapper, visited)) = pending.pop() {
            if !visited {
                pending.push((wrapper, true));
                for parameter in &wrapper.params {
                    if let ValType::Literal(TextForgeParamTypes::Token(child)) = parameter {
                        pending.push((child, false));
                    }
                }
                continue;
            }
            let syntax = match TOKEN_TABLE.find((
                QuerySource::Identifier(wrapper.get_string_repr().into()),
                QueryTarget::Syntax,
            ))? {
                TargetValue::Syntax(syntax) => syntax,
                _ => {
                    return Err(TextForgeError::new(
                        TextForgeErrorCode::SerializationError(
                            format!(
                                "internal error: `{}` has no registered syntax",
                                wrapper.get_string_repr()
                            )
                            .into(),
                        ),
                        "",
                        wrapper.get_string_repr(),
                    ));
                }
            };
            let mut parameters = wrapper.params.iter();
            let mut fields = vec![wrapper.get_string_repr().to_owned()];
            for part in syntax.iter() {
                if let SyntaxToken::Literal(literal) = part.token {
                    fields.push(literal.to_owned());
                    continue;
                }
                // A joined pipeline has no text form: `ppjn "path";`.
                if matches!(part.token, SyntaxToken::Pipeline) {
                    parameters.next();
                    continue;
                }
                let parameter = parameters.next().ok_or_else(|| {
                    TextForgeError::new(
                        TextForgeErrorCode::SerializationError(
                            format!(
                                "`{}` has fewer parameters than its syntax needs",
                                wrapper.get_string_repr()
                            )
                            .into(),
                        ),
                        "",
                        wrapper.get_string_repr(),
                    )
                })?;
                let value = match parameter {
                    ValType::Literal(TextForgeParamTypes::Pipeline(_)) => {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::SerializationError(
                                "internal error: a joined pipeline is where its syntax expects text".into(),
                            ),
                            "",
                            wrapper.get_string_repr(),
                        ));
                    }
                    ValType::Literal(TextForgeParamTypes::Token(_)) => {
                        output.pop().ok_or_else(|| {
                            TextForgeError::new(
                                TextForgeErrorCode::SerializationError(
                                    "internal error: the nested instruction was not serialized"
                                        .into(),
                                ),
                                "",
                                wrapper.get_string_repr(),
                            )
                        })?
                    }
                    ValType::Literal(TextForgeParamTypes::Usize(value)) => value.to_string(),
                    ValType::VarRef(name) | ValType::Literal(TextForgeParamTypes::VarRef(name)) => {
                        let reference = format!("{{{{{name}}}}}");
                        if !VAR_REF_RE.is_match(&reference) {
                            return Err(TextForgeError::new(
                                TextForgeErrorCode::SerializationError(
                                    format!("the variable name `{name}` cannot be written in the text format (a name is a letter or `_` followed by letters, digits or `_`); save the pipeline as bytecode instead").into(),
                                ),
                                "", name.clone(),
                            ));
                        }
                        crate::text::line_parser::quote(&reference).into_owned()
                    }
                    ValType::Literal(TextForgeParamTypes::String(value)) => {
                        // Line breaks are written as escapes in double quotes.
                        crate::text::line_parser::quote(value).into_owned()
                    }
                };
                fields.push(value);
            }
            output.push(fields.join(" "));
        }
        let text = output.pop().ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::SerializationError(
                    "internal error: the instruction produced no text".into(),
                ),
                "",
                "",
            )
        })?;
        Ok(format!("{text};\n"))
    }
    #[cfg(feature = "bytecode")]
    pub fn to_bytecode_resolved(
        &self,
        context: &mut GlobalExecutionContext,
    ) -> Result<Vec<u8>, TextForgeError> {
        let parsed_params = ValType::resolve_variables(
            self.token.as_ref(),
            &self.params,
            self.compiled.as_deref(),
            context,
            CurrentText::Unavailable,
        )?;
        let mut t = self.token.clone();
        t.set_params(&parsed_params)?;

        t.to_bytecode()
    }
    #[cfg(feature = "bytecode")]
    pub fn to_bytecode_unresolved(&self) -> Result<Vec<u8>, TextForgeError> {
        let mut unresolved_params: Vec<TextForgeParamTypes> = Vec::new();
        for val in self.params.iter() {
            match val {
                ValType::Literal(x) => unresolved_params.push(x.clone()),
                ValType::VarRef(name) => {
                    unresolved_params.push(TextForgeParamTypes::VarRef(name.to_string()));
                }
            }
        }

        // This persistence path must not resolve nested references. The public
        // context-aware parameter writer keeps its existing resolved semantics.
        let count = u8::try_from(unresolved_params.len()).map_err(|_| {
            TextForgeError::new(
                TextForgeErrorCode::BytecodeParsingError(
                    "an instruction with more than 255 parameters cannot be saved as bytecode"
                        .into(),
                ),
                "bytecode",
                "",
            )
        })?;
        let mut body = Vec::new();
        body.extend_from_slice(&self.get_opcode().to_be_bytes());
        body.push(count);
        for parameter in &unresolved_params {
            parameter.write_unresolved_instruction_param(&mut body)?;
        }
        let mut result = Vec::with_capacity(8 + body.len());
        result.extend_from_slice(&(body.len() as u64).to_be_bytes());
        result.extend_from_slice(&body);
        Ok(result)
    }
}

/// What a parameter slot needs, for error messages.
fn describe_syntax(token: SyntaxToken) -> &'static str {
    match token {
        SyntaxToken::String => "text",
        SyntaxToken::Usize => "a non-negative integer",
        SyntaxToken::Token => "an instruction",
        SyntaxToken::Pipeline => "a joined pipeline",
        SyntaxToken::Literal(_) => "a keyword",
    }
}

pub fn get_effective_param_types(expected: &[SyntaxDef]) -> Vec<SyntaxToken> {
    expected
        .iter()
        .filter_map(|ip| match ip.token {
            SyntaxToken::Literal(_) => None,
            other => Some(other),
        })
        .collect()
}

impl ValType {
    fn coerce_param(
        value: TextForgeParamTypes,
        expected: SyntaxToken,
        context_label: &str,
    ) -> Result<TextForgeParamTypes, TextForgeError> {
        match (value, expected) {
            (TextForgeParamTypes::String(v), SyntaxToken::String) => {
                Ok(TextForgeParamTypes::String(v))
            }
            (TextForgeParamTypes::Usize(v), SyntaxToken::Usize) => {
                Ok(TextForgeParamTypes::Usize(v))
            }
            (TextForgeParamTypes::Token(v), SyntaxToken::Token) => {
                Ok(TextForgeParamTypes::Token(v))
            }

            // Lenient coercion: string <-> number
            (TextForgeParamTypes::String(s), SyntaxToken::Usize) => s
                .parse::<usize>()
                .map(TextForgeParamTypes::Usize)
                .map_err(|_| {
                    TextForgeError::new(
                        TextForgeErrorCode::IncompatibleTypeError(
                            format!(
                                "`{context_label}` gives \"{s}\", but this parameter needs a non-negative integer"
                            )
                            .into(),
                        ),
                        "",
                        "",
                    )
                }),
            (TextForgeParamTypes::Usize(n), SyntaxToken::String) => {
                Ok(TextForgeParamTypes::String(n.to_string()))
            }

            // A Token has no sensible conversion from String/Usize; this also covers
            // the degenerate case of an unresolved TextForgeParamTypes::VarRef
            // reaching this point (it should not happen, but this is a safe catch-all).
            (_, expected) => Err(TextForgeError::new(
                TextForgeErrorCode::IncompatibleTypeError(
                    format!(
                        "`{context_label}` cannot be used here: this parameter needs {}",
                        describe_syntax(expected)
                    )
                    .into(),
                ),
                "",
                "",
            )),
        }
    }
    /// Bind parameters: interpolate templates and read variable references.
    /// `text` is the text the instruction will receive; text-derived built-ins
    /// are computed from it only if a parameter reads them.
    fn resolve_variables(
        t: &dyn InstructionMethods,
        values: &[ValType],
        compiled: Option<&CompiledParams>,
        context: &GlobalExecutionContext,
        text: CurrentText,
    ) -> Result<Vec<TextForgeParamTypes>, TextForgeError> {
        let reader = VarReader::new(context, text);
        let looked_up;
        let expected_params: &[SyntaxToken] = match compiled {
            Some(compiled) => &compiled.expected,
            None => {
                looked_up = Self::expected_params(t)?;
                &looked_up
            }
        };
        let mut result = Vec::with_capacity(values.len());
        // A string parameter: its compiled template, or interpolation.
        let render =
            |index: usize, text: &str| match compiled.and_then(|c| c.templates[index].as_ref()) {
                Some(template) => template.render(&reader),
                None if compiled.is_some() => Ok(text.to_owned()),
                None => interpolate_with(text, &reader),
            };

        if values.len() != expected_params.len() {
            return Err(TextForgeError::new(
                TextForgeErrorCode::InvalidParameters(
                    format!(
                        "`{}` takes {}, but {} given",
                        t.get_string_repr(),
                        crate::utils::errors::plural(expected_params.len(), "parameter"),
                        if values.len() == 1 {
                            "1 was".to_owned()
                        } else {
                            format!("{} were", values.len())
                        }
                    )
                    .into(),
                ),
                "",
                "",
            ));
        }
        Self::bind(values, expected_params, &reader, render, &mut result)?;
        Ok(result)
    }

    fn expected_params(t: &dyn InstructionMethods) -> Result<Vec<SyntaxToken>, TextForgeError> {
        let query_result = TOKEN_TABLE.find((
            QuerySource::Identifier(t.get_string_repr().into()),
            QueryTarget::Syntax,
        ))?;

        Ok(get_effective_param_types(
            &(match query_result {
                TargetValue::Syntax(x) => x,
                _ => unreachable!("Unreachable Code"),
            }),
        ))
    }

    fn bind(
        values: &[ValType],
        expected_params: &[SyntaxToken],
        reader: &VarReader,
        render: impl Fn(usize, &str) -> Result<String, TextForgeError>,
        result: &mut Vec<TextForgeParamTypes>,
    ) -> Result<(), TextForgeError> {
        for (i, value) in values.iter().enumerate() {
            match value {
                ValType::Literal(literal) => match (literal, expected_params[i]) {
                    // The only arm that differs: a string literal is interpolated
                    // before it becomes the final parameter. This covers both the
                    // common case (no {{}}/[[]] present, so
                    // interpolate() returns the string unchanged on its
                    // fast path) and strings with any number of markers.
                    (TextForgeParamTypes::String(s), SyntaxToken::String) => {
                        result.push(TextForgeParamTypes::String(render(i, s)?));
                    }

                    // A numeric parameter written as an expression
                    // (`[[__COUNTER__ + 1]]`): interpolate, then parse.
                    (TextForgeParamTypes::String(s), SyntaxToken::Usize)
                        if is_numeric_template(s) =>
                    {
                        let text = render(i, s)?;
                        result.push(Self::coerce_param(
                            TextForgeParamTypes::String(text),
                            SyntaxToken::Usize,
                            s,
                        )?);
                    }

                    (TextForgeParamTypes::Usize(_), SyntaxToken::Usize)
                    | (TextForgeParamTypes::Token(_), SyntaxToken::Token)
                    | (TextForgeParamTypes::Pipeline(_), SyntaxToken::Pipeline) => {
                        result.push(literal.clone());
                    }

                    _ => {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::IncompatibleTypeError(
                                format!(
                                    "parameter {} must be {}, but it is {}",
                                    i + 1,
                                    describe_syntax(expected_params[i]),
                                    literal.kind_name()
                                )
                                .into(),
                            ),
                            "",
                            "",
                        ));
                    }
                },

                ValType::VarRef(name) => {
                    let value = reader.value(name)?.into_owned();

                    let coerced = Self::coerce_param(value.into(), expected_params[i], name)?;

                    result.push(coerced);
                }
            }
        }
        Ok(())
    }
}

#[cfg(all(test, feature = "test_access"))]
mod preparation_test;
