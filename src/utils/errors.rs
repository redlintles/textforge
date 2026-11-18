//! [`TextForgeError`], its [`TextForgeErrorCode`] and the [`ErrorManager`]
//! that collects errors during processing.
use std::borrow::Cow;
use std::error::Error;
use std::fmt::Display;

// Colored formatting (presentation only)
use colored::{Color, Colorize};

/// The error of an instruction run without an execution context (for example
/// through `InstructionMethods::transform(input, None)`): `purpose` says what
/// the context is needed for.
pub(crate) fn context_required(instruction: &'static str, purpose: &str) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::RequiredContextError(
            format!(
                "`{instruction}` needs an execution context to {purpose}; run it in a pipeline (TextForgeProcessor) instead of calling transform() without a context"
            )
            .into(),
        ),
        instruction,
        "",
    )
}

/// The error for a pattern that is not a valid regular expression.
pub(crate) fn regex_error(
    instruction: &'static str,
    pattern: &str,
    error: impl Display,
) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::TextParsingError(
            format!("`{pattern}` is not a valid regular expression").into(),
        ),
        instruction,
        pattern.to_owned(),
    )
    .with_cause(error)
}

/// `count` followed by `word`, in the plural unless `count` is 1.
pub(crate) fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("1 {word}")
    } else {
        format!("{count} {word}s")
    }
}

/// Characters of the processed text an error keeps in its `input` field.
pub const INPUT_EXCERPT_CHARS: usize = 80;

/// The beginning of `text` for an error's `input` field: at most
/// [`INPUT_EXCERPT_CHARS`] characters, followed by `… [N bytes]` when the
/// text is longer. Errors about a position in the document use this instead of
/// copying the whole document, which could be hundreds of megabytes.
///
/// ```
/// use textforge::utils::errors::input_excerpt;
/// assert_eq!(input_excerpt("short"), "short");
/// let long = "a".repeat(1000);
/// assert_eq!(input_excerpt(&long), format!("{}… [1000 bytes]", "a".repeat(80)));
/// ```
pub fn input_excerpt(text: &str) -> String {
    match text.char_indices().nth(INPUT_EXCERPT_CHARS) {
        None => text.to_owned(),
        Some((cut, _)) => format!("{}… [{} bytes]", &text[..cut], text.len()),
    }
}

/// Collects the errors reported while a processor works; it can also print
/// them or panic on the first one.
///
/// Deprecated since 0.4.0: the processor no longer records into it (it kept
/// every error for the lifetime of the processor, and nothing read them).
/// Errors ignored by [`tryi`](crate::tokens::instructions::tryi::Tryi) are
/// kept per run and shown by the debug methods; render an error with
/// [`TextForgeError::report`].
#[deprecated(
    since = "0.4.0",
    note = "the processor no longer records errors here; use the error a method returns and TextForgeError::report"
)]
#[derive(Default, Clone)]
pub struct ErrorManager {
    panic_with_error: bool,
    show_warnings: bool,
    error_vec: Vec<TextForgeError>,
}

/// An error: what went wrong ([`TextForgeErrorCode`]), where (the instruction or
/// stage) and on what (a parameter, a path or an excerpt of the text).
///
/// Errors raised while a pipeline runs also carry [`details`](Self::details):
/// the instructions they passed through, the parameters of the instruction
/// that failed and the cause. [`Display`] is one plain line;
/// [`report`](Self::report) renders everything, in color on a terminal.
///
/// ```
/// use textforge::api::{processor::{TextForgeProcessor, TextForgeProcessorMethods}, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().delete_chunk(9, 9)?.build()?;
/// let error = processor.process_all(&id, "a").unwrap_err();
/// assert_eq!(error.error_code.get_error_code(), 12);
/// assert!(error.to_string().starts_with("error[12] IndexOutOfRange: "));
/// assert_eq!(error.parameters()[0].name, "start");
/// assert_eq!(error.parameters()[0].value, "9");
/// let report = error.report().color(false).to_string();
/// assert!(report.contains("start = 9"));
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone)]
pub struct TextForgeError {
    /// What went wrong, with a message.
    pub error_code: TextForgeErrorCode,
    /// The instruction or stage that reported the error.
    pub instruction: Cow<'static, str>,
    /// What the error is about: a parameter, a path or an excerpt of the text
    /// (see [`input_excerpt`]).
    pub input: Cow<'static, str>,
    /// What the run added while the error left the pipeline: trace,
    /// parameters, cause. `None` for errors that did not pass through a
    /// running instruction (and boxed, so a `Result` stays small).
    pub details: Option<Box<ErrorDetails>>,
}

/// The fields, with `details` only when there are some (so an error without
/// them prints as before 0.4.0).
impl std::fmt::Debug for TextForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = f.debug_struct("TextForgeError");
        debug
            .field("error_code", &self.error_code)
            .field("instruction", &self.instruction)
            .field("input", &self.input);
        if let Some(details) = &self.details {
            debug.field("details", details);
        }
        debug.finish()
    }
}

/// Two errors are equal when their code, message, instruction and input are:
/// [`details`](TextForgeError::details) are diagnostics and are not compared.
impl PartialEq for TextForgeError {
    fn eq(&self, other: &Self) -> bool {
        self.error_code == other.error_code
            && self.instruction == other.instruction
            && self.input == other.input
    }
}

/// What an error passed through and worked on, collected while it leaves the
/// pipeline (only on the error path: a run that succeeds builds none of it).
/// See [`TextForgeError::trace`], [`TextForgeError::parameters`] and
/// [`TextForgeError::cause`].
///
/// ```
/// use textforge::utils::errors::{TextForgeError, TextForgeErrorCode};
/// let error = TextForgeError::new(TextForgeErrorCode::FileReadingError("cannot read `a.txt`".into()), "apf", "a.txt")
///     .with_cause("permission denied");
/// assert_eq!(error.details.as_ref().unwrap().cause.as_deref(), Some("permission denied"));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErrorDetails {
    /// The instructions the error passed through, **innermost first** (the
    /// one that failed, then the `iter`, block or joined pipeline around it).
    pub frames: Vec<TraceFrame>,
    /// The parameters of the instruction that failed, with the values it ran
    /// with.
    pub parameters: Vec<ErrorParameter>,
    /// The lower-level error behind this one (the operating system's I/O
    /// error, the regex error, …).
    pub cause: Option<String>,
    /// A note for the next frame, set by the container that knows it (the
    /// iteration of `iter`, the step of a block).
    pending_note: Option<String>,
}

/// One instruction an error passed through. See [`ErrorDetails::frames`].
///
/// ```
/// use textforge::utils::errors::TraceFrame;
/// let frame = TraceFrame::new("dlc 9 9;", Some("clean.textforge:3".into()), None);
/// assert_eq!(frame.location.as_deref(), Some("clean.textforge:3"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct TraceFrame {
    /// The instruction as it ran (the failing one with its resolved values).
    pub instruction: String,
    /// `file:line` of the instruction, when the pipeline came from a file.
    pub location: Option<String>,
    /// What the container was doing (`__COUNTER__ = 1 (iteration 2 of 3)`).
    pub note: Option<String>,
}

impl TraceFrame {
    /// A frame for `instruction`, at `location`, with an optional note.
    pub fn new(
        instruction: impl Into<String>,
        location: Option<String>,
        note: Option<String>,
    ) -> Self {
        Self {
            instruction: instruction.into(),
            location,
            note,
        }
    }
}

/// One parameter of the instruction that failed. See
/// [`ErrorDetails::parameters`].
///
/// ```
/// use textforge::utils::errors::ErrorParameter;
/// let parameter = ErrorParameter::new("start", "9", Some("[[n + 1]]".into()));
/// assert_eq!(parameter.written.as_deref(), Some("[[n + 1]]"));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErrorParameter {
    /// The name of the parameter in the instruction's syntax (`start`), or
    /// `parameter N` when it has none.
    pub name: String,
    /// The value the instruction ran with, after interpolation.
    pub value: String,
    /// The parameter as written in the pipeline, when it differs from the
    /// value (a variable or an expression).
    pub written: Option<String>,
}

impl ErrorParameter {
    /// A parameter `name` with `value`, written as `written` when it differs.
    pub fn new(name: impl Into<String>, value: impl Into<String>, written: Option<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            written,
        }
    }
}

impl Error for TextForgeError {}

/// One plain line, without color: `error[12] IndexOutOfRange: message (in:
/// instruction)`. Use [`TextForgeError::report`] for the full report.
impl Display for TextForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error_code)?;
        if !self.instruction.is_empty() {
            write!(f, " (in: {})", self.instruction)?;
        }
        Ok(())
    }
}

impl TextForgeError {
    /// Builds an error from its code, the reporting instruction and its input.
    pub fn new<I, T>(error_code: TextForgeErrorCode, instruction: I, input: T) -> Self
    where
        I: Into<Cow<'static, str>>,
        T: Into<Cow<'static, str>>,
    {
        TextForgeError {
            error_code,
            instruction: instruction.into(),
            input: input.into(),
            details: None,
        }
    }

    // Cheap helpers (do not change public fields; just convenience)
    /// The reporting instruction, as `&str`.
    pub fn instruction_str(&self) -> &str {
        self.instruction.as_ref()
    }
    /// The input, as `&str`.
    pub fn input_str(&self) -> &str {
        self.input.as_ref()
    }

    /// The details, created empty when the error has none yet.
    pub(crate) fn details_mut(&mut self) -> &mut ErrorDetails {
        self.details.get_or_insert_with(Box::default)
    }

    /// The instructions the error passed through, **outermost first**, the
    /// last one being the instruction that failed. Empty for errors that were
    /// not raised by a running instruction.
    pub fn trace(&self) -> impl DoubleEndedIterator<Item = &TraceFrame> {
        self.details
            .iter()
            .flat_map(|details| details.frames.iter().rev())
    }

    /// The parameters of the instruction that failed (empty when unknown).
    pub fn parameters(&self) -> &[ErrorParameter] {
        self.details
            .as_ref()
            .map_or(&[], |details| details.parameters.as_slice())
    }

    /// The lower-level error behind this one, when there is one.
    pub fn cause(&self) -> Option<&str> {
        self.details.as_ref()?.cause.as_deref()
    }

    /// This error, caused by `cause` (an I/O error, a regex error, …).
    pub fn with_cause(mut self, cause: impl Display) -> Self {
        self.details_mut().cause = Some(cause.to_string());
        self
    }

    /// Whether [`tryi`](crate::tokens::instructions::tryi::Tryi) may ignore
    /// this error: every error an instruction raises while it runs, except a
    /// safety limit ([`TextForgeErrorCode::LimitExceededError`]).
    pub fn is_ignorable(&self) -> bool {
        self.error_code.ignorable()
    }

    /// The full report: kind, message, trace, parameters, input, cause and a
    /// hint, in color when the terminal supports it (see [`ErrorReport`]).
    pub fn report(&self) -> ErrorReport<'_> {
        ErrorReport {
            error: self,
            ignored: false,
            color: colored::control::SHOULD_COLORIZE.should_colorize(),
        }
    }

    /// Note what the enclosing container was doing (the iteration, the step),
    /// for the container's own frame; an inner container's note is kept.
    pub(crate) fn with_note(mut self, note: impl FnOnce() -> String) -> Self {
        let details = self.details_mut();
        if details.pending_note.is_none() {
            details.pending_note = Some(note());
        }
        self
    }

    /// Add the frame of an instruction the error is leaving (innermost
    /// first), with the pending note, and its parameters when the instruction
    /// is the one that failed.
    pub(crate) fn push_frame(
        &mut self,
        instruction: String,
        location: Option<String>,
        parameters: impl FnOnce() -> Vec<ErrorParameter>,
    ) {
        let details = self.details_mut();
        let note = details.pending_note.take();
        if details.frames.is_empty() && details.parameters.is_empty() {
            details.parameters = parameters();
        }
        details.frames.push(TraceFrame {
            instruction,
            location,
            note,
        });
    }
}

/// The report of a [`TextForgeError`], from [`TextForgeError::report`]: a
/// traceback in the manner of Python and Java, for people. Its layout may
/// change between versions; programs should read the error's fields.
///
/// ```
/// use textforge::utils::errors::{TextForgeError, TextForgeErrorCode};
/// let error = TextForgeError::new(
///     TextForgeErrorCode::IndexOutOfRange("index 9 is past the end".into()), "dls 9;", "abc",
/// );
/// let report = error.report().color(false).to_string();
/// assert!(report.starts_with("error[12] IndexOutOfRange: index 9 is past the end"));
/// assert!(report.contains("--> dls 9;"));
/// assert!(error.report().ignored(true).color(false).to_string().starts_with("warning[12]"));
/// ```
pub struct ErrorReport<'a> {
    error: &'a TextForgeError,
    ignored: bool,
    color: bool,
}

/// Widest instruction shown in a trace line.
const MAX_TRACE_INSTRUCTION_CHARS: usize = 70;

impl ErrorReport<'_> {
    /// Report the error as one `tryi` ignored: a `warning`, and what happened
    /// instead.
    pub fn ignored(mut self, ignored: bool) -> Self {
        self.ignored = ignored;
        self
    }

    /// Color the report (`true`) or write plain text (`false`). By default it
    /// follows `colored` (`NO_COLOR`, `CLICOLOR`, whether stdout is a terminal).
    pub fn color(mut self, color: bool) -> Self {
        self.color = color;
        self
    }

    /// `text` in the SGR style `style` (`"1;31"`: bold red) when coloring.
    /// Written directly, so `color(true)` colors whatever `colored` decides.
    fn paint(&self, text: &str, style: &str) -> String {
        if self.color {
            format!("\x1b[{style}m{text}\x1b[0m")
        } else {
            text.to_owned()
        }
    }
}

/// The SGR foreground code of `color`.
fn sgr(color: Color) -> &'static str {
    match color {
        Color::Red => "31",
        Color::Yellow => "33",
        Color::Magenta => "35",
        _ => "39",
    }
}

fn shorten(text: &str, max: usize) -> String {
    let text = text.trim_end_matches(['\n', ';']).trim_end();
    match text.char_indices().nth(max) {
        None => format!("{text};"),
        Some((cut, _)) => format!("{}…", &text[..cut]),
    }
}

impl Display for ErrorReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error = self.error;
        let code = &error.error_code;
        let (word, color) = if self.ignored {
            ("warning", Color::Yellow)
        } else {
            ("error", code.severity_color())
        };
        let label = format!("{word}[{}]", code.get_error_code());
        write!(
            f,
            "{} {}",
            self.paint(&label, &format!("1;{}", sgr(color))),
            self.paint(code.name(), "1")
        )?;
        if self.ignored {
            write!(f, " {}", self.paint("(ignored by tryi)", "33"))?;
        }
        write!(f, ": {}", code.message())?;
        if !error.instruction.is_empty() {
            write!(f, "\n  {} {}", self.paint("-->", "1;36"), error.instruction)?;
        }

        let frames: Vec<&TraceFrame> = error.trace().collect();
        if !frames.is_empty() {
            // An inner instruction is on its container's source line.
            let mut locations = Vec::with_capacity(frames.len());
            let mut last = None;
            for frame in &frames {
                if frame.location.is_some() {
                    last = frame.location.clone();
                }
                locations.push(last.clone().unwrap_or_default());
            }
            let width = locations
                .iter()
                .map(|l| l.chars().count())
                .max()
                .unwrap_or(0);
            write!(f, "\n\n  {}", self.paint("Trace (outermost first):", "1"))?;
            for (index, (frame, location)) in frames.iter().zip(&locations).enumerate() {
                let failed = index + 1 == frames.len();
                let arrow = if failed {
                    self.paint("→", "1;31")
                } else {
                    " ".to_owned()
                };
                let location = format!("{location:<width$}");
                write!(
                    f,
                    "\n    {arrow} {}{}{}",
                    if width > 0 {
                        format!("{}  ", self.paint(&location, "2"))
                    } else {
                        String::new()
                    },
                    self.paint(
                        &shorten(&frame.instruction, MAX_TRACE_INSTRUCTION_CHARS),
                        "33"
                    ),
                    match (&frame.note, failed) {
                        (Some(note), _) => format!("  {}", self.paint(&format!("· {note}"), "2")),
                        (None, true) => format!("  {}", self.paint("← failed here", "31")),
                        (None, false) => String::new(),
                    }
                )?;
            }
        }

        let parameters = error.parameters();
        if !parameters.is_empty() {
            let width = parameters
                .iter()
                .map(|p| p.name.chars().count())
                .max()
                .unwrap_or(0);
            write!(f, "\n\n  {}", self.paint("Parameters:", "1"))?;
            for parameter in parameters {
                let name = format!("{:<width$}", parameter.name);
                write!(f, "\n    {} = {}", self.paint(&name, "36"), parameter.value)?;
                if let Some(written) = &parameter.written {
                    write!(f, "   {}", self.paint(&format!("(written {written})"), "2"))?;
                }
            }
        }

        if !error.input.is_empty() {
            write!(
                f,
                "\n\n  {} {}",
                self.paint("Input:", "1"),
                self.paint(&error.input.escape_debug().to_string(), "2")
            )?;
        }
        if let Some(cause) = error.cause() {
            write!(f, "\n\n  {} {cause}", self.paint("Caused by:", "1;33"))?;
        }
        if let Some(hint) = code.hint() {
            write!(f, "\n\n  {} {hint}", self.paint("Hint:", "1;32"))?;
        }
        if self.ignored {
            write!(
                f,
                "\n\n  {}",
                self.paint(
                    "The instruction was skipped: the text and the variables are as they were before it, and the pipeline continued.",
                    "33",
                )
            )?;
        }
        Ok(())
    }
}

#[allow(deprecated)]
impl ErrorManager {
    /// Panic on the next error added.
    pub fn will_panic(&mut self, val: bool) {
        self.panic_with_error = val;
    }

    /// Print each error added (and enable colored output).
    pub fn will_log(&mut self, val: bool) {
        self.show_warnings = val;

        // Tie "log mode" to color output by default.
        colored::control::set_override(val);
    }

    /// Records an error.
    pub fn add_error(&mut self, err: TextForgeError) {
        self.error_vec.push(err);
    }

    /// Whether any error was recorded.
    pub fn has_errors(&self) -> bool {
        !self.error_vec.is_empty()
    }

    /// Optional: reserve capacity when you expect many errors in a big pipeline.
    pub fn reserve_errors(&mut self, additional: usize) {
        self.error_vec.reserve(additional);
    }

    /// Keeps existing API: prints to stdout.
    /// Internally uses a writer so it can be unit-tested and buffered.
    pub fn print_errors(&self) {
        use std::io::{self, BufWriter};
        let stdout = io::stdout();
        let mut w = BufWriter::new(stdout.lock());
        let _ = self.write_errors(&mut w);
    }

    /// Internal helper: write formatted errors into any writer (test-friendly).
    fn write_errors<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
        for (index, error) in self.error_vec.iter().enumerate() {
            writeln!(
                w,
                "[{}] {} {}\n\t{} {}\n\t{} {}\n\t{} {}",
                index.to_string().blue(),
                "Code:".bold().red(),
                error.error_code.get_error_code().to_string().red().bold(),
                "Instruction:".bold().cyan(),
                error.instruction.as_ref().cyan(),
                "Message:".bold().yellow(),
                // IMPORTANT: avoid allocating; get_message returns &Cow now.
                error.error_code.get_message().as_ref().yellow(),
                "Processing:".bold().dimmed(),
                error.input.as_ref().dimmed()
            )?;
        }
        Ok(())
    }

    /// Optional: consistent behavior when you want to both store and act on an error.
    /// Does not change any existing behavior unless you choose to use it.
    pub fn handle_error(&mut self, err: TextForgeError) {
        self.add_error(err.clone());

        if self.show_warnings {
            println!("{err}");
        }

        if self.panic_with_error {
            panic!("{err}");
        }
    }
}

/// The kind of an error, each with a message.
/// [`get_error_code`](Self::get_error_code) gives its stable number.
#[derive(Clone, Debug, PartialEq)]
pub enum TextForgeErrorCode {
    /// A file does not exist.
    FileNotFound(Cow<'static, str>),
    /// A bytecode parameter has an unknown type.
    BytecodeParamNotRecognized(Cow<'static, str>),
    /// An unknown instruction mnemonic or opcode.
    TokenNotFound(Cow<'static, str>),
    /// An unknown pipeline identifier.
    TokenArrayNotFound(Cow<'static, str>),
    /// `cblk` names a block that was not defined.
    BlockNotFound(Cow<'static, str>),
    /// A variable that was not defined.
    VariableNotFound(Cow<'static, str>),
    /// An attempt to change a variable defined with `val`.
    NonMutableVariableError(Cow<'static, str>),
    /// A file could not be read.
    FileReadingError(Cow<'static, str>),
    /// A file could not be written.
    FileWritingError(Cow<'static, str>),
    /// A file could not be opened.
    FileOpeningError(Cow<'static, str>),
    /// A bytecode file or record is missing.
    BytecodeNotFound(Cow<'static, str>),
    /// A `.textforge` line could not be parsed.
    TextParsingError(Cow<'static, str>),
    /// A bytecode file is malformed.
    BytecodeParsingError(Cow<'static, str>),
    /// A bytecode parameter is malformed.
    BytecodeParamParsingError(Cow<'static, str>),
    /// An index that is not valid for the instruction.
    InvalidIndex(Cow<'static, str>),
    /// An index or line past the end of the text.
    IndexOutOfRange(Cow<'static, str>),
    /// Operands an operation cannot use.
    InvalidOperands(Cow<'static, str>),
    /// Parameters an instruction cannot use.
    InvalidParameters(Cow<'static, str>),
    /// A rule was broken: a reserved name, a nesting limit, a safety limit, and so on.
    ValidationError(Cow<'static, str>),
    /// The wrong number of parameters.
    InvalidArgumentNumber(Cow<'static, str>),
    /// A division by zero.
    ZeroDivisionError(Cow<'static, str>),
    /// A value could not be converted to the expected type.
    TryIntoFailError(Cow<'static, str>),
    /// A value of the wrong type for a parameter.
    IncompatibleTypeError(Cow<'static, str>),
    /// An instruction that needs an execution context ran without one.
    RequiredContextError(Cow<'static, str>),
    /// An unknown watcher name.
    WatcherNotFoundError(Cow<'static, str>),
    /// A pipeline or report could not be serialized.
    SerializationError(Cow<'static, str>),
    /// Any other error.
    GenericError(Cow<'static, str>),
    /// A block defined inside a block.
    NestedBlocksNotAllowedError(Cow<'static, str>),
    /// Bytes that are not valid UTF-8.
    InvalidUtf8Error(Cow<'static, str>),
    /// An expression that cannot be parsed or evaluated.
    InvalidExprError(Cow<'static, str>),
    /// A safety limit was reached (`max_iterations`, `max_generated_bytes`):
    /// the only error [`tryi`](crate::tokens::instructions::tryi::Tryi) never
    /// ignores. Before 0.4.0 it was a [`ValidationError`](Self::ValidationError).
    LimitExceededError(Cow<'static, str>),
}

/// One plain line: `error[12] IndexOutOfRange: message`.
impl Display for TextForgeErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "error[{}] {}: {}",
            self.get_error_code(),
            self.name(),
            self.message()
        )
    }
}

impl TextForgeErrorCode {
    /// The stable number of this kind of error (1–31).
    pub fn get_error_code(&self) -> u16 {
        match self {
            Self::FileNotFound(_) => 1u16,
            Self::TokenNotFound(_) => 2u16,
            Self::TokenArrayNotFound(_) => 3u16,
            Self::FileReadingError(_) => 4u16,
            Self::FileWritingError(_) => 5u16,
            Self::FileOpeningError(_) => 6u16,
            Self::BytecodeNotFound(_) => 7u16,
            Self::BlockNotFound(_) => 8u16,
            Self::VariableNotFound(_) => 9u16,
            Self::NonMutableVariableError(_) => 10u16,
            Self::InvalidOperands(_) => 11u16,
            Self::IndexOutOfRange(_) => 12u16,
            Self::InvalidIndex(_) => 13u16,
            Self::InvalidParameters(_) => 14u16,
            Self::InvalidArgumentNumber(_) => 15u16,
            Self::BytecodeParamNotRecognized(_) => 16u16,
            Self::TextParsingError(_) => 17u16,
            Self::BytecodeParsingError(_) => 18u16,
            Self::BytecodeParamParsingError(_) => 19u16,
            Self::ValidationError(_) => 20u16,
            Self::ZeroDivisionError(_) => 21u16,
            Self::TryIntoFailError(_) => 22u16,
            Self::IncompatibleTypeError(_) => 23u16,
            Self::RequiredContextError(_) => 24u16,
            Self::WatcherNotFoundError(_) => 25u16,
            Self::SerializationError(_) => 26u16,
            Self::GenericError(_) => 27u16,
            Self::NestedBlocksNotAllowedError(_) => 28u16,
            Self::InvalidUtf8Error(_) => 29u16,
            Self::InvalidExprError(_) => 30u16,
            Self::LimitExceededError(_) => 31u16,
        }
    }

    /// The stable name of this kind of error, as reports show it
    /// (`IndexOutOfRange`).
    ///
    /// ```
    /// use textforge::utils::errors::TextForgeErrorCode;
    /// assert_eq!(TextForgeErrorCode::IndexOutOfRange("".into()).name(), "IndexOutOfRange");
    /// ```
    pub fn name(&self) -> &'static str {
        match self {
            Self::FileNotFound(_) => "FileNotFound",
            Self::TokenNotFound(_) => "TokenNotFound",
            Self::TokenArrayNotFound(_) => "TokenArrayNotFound",
            Self::FileReadingError(_) => "FileReadingError",
            Self::FileWritingError(_) => "FileWritingError",
            Self::FileOpeningError(_) => "FileOpeningError",
            Self::BytecodeNotFound(_) => "BytecodeNotFound",
            Self::BlockNotFound(_) => "BlockNotFound",
            Self::VariableNotFound(_) => "VariableNotFound",
            Self::NonMutableVariableError(_) => "NonMutableVariableError",
            Self::InvalidOperands(_) => "InvalidOperands",
            Self::IndexOutOfRange(_) => "IndexOutOfRange",
            Self::InvalidIndex(_) => "InvalidIndex",
            Self::InvalidParameters(_) => "InvalidParameters",
            Self::InvalidArgumentNumber(_) => "InvalidArgumentNumber",
            Self::BytecodeParamNotRecognized(_) => "BytecodeParamNotRecognized",
            Self::TextParsingError(_) => "TextParsingError",
            Self::BytecodeParsingError(_) => "BytecodeParsingError",
            Self::BytecodeParamParsingError(_) => "BytecodeParamParsingError",
            Self::ValidationError(_) => "ValidationError",
            Self::ZeroDivisionError(_) => "ZeroDivisionError",
            Self::TryIntoFailError(_) => "TryIntoFailError",
            Self::IncompatibleTypeError(_) => "IncompatibleTypeError",
            Self::RequiredContextError(_) => "RequiredContextError",
            Self::WatcherNotFoundError(_) => "WatcherNotFoundError",
            Self::SerializationError(_) => "SerializationError",
            Self::GenericError(_) => "GenericError",
            Self::NestedBlocksNotAllowedError(_) => "NestedBlocksNotAllowedError",
            Self::InvalidUtf8Error(_) => "InvalidUtf8Error",
            Self::InvalidExprError(_) => "InvalidExprError",
            Self::LimitExceededError(_) => "LimitExceededError",
        }
    }

    /// Whether [`tryi`](crate::tokens::instructions::tryi::Tryi) may ignore
    /// this kind of error when the instruction it wraps raises it. Only the
    /// safety limits are not ignorable: the user sets them for the whole run
    /// (`.txfrc`, `txf` flags, `ExecutionProfile`), not the pipeline. Errors
    /// of loading (parse, validation, bytecode) never reach `tryi`, since they
    /// happen before the pipeline runs.
    ///
    /// The match has no wildcard on purpose: a new code must decide.
    ///
    /// ```
    /// use textforge::utils::errors::TextForgeErrorCode;
    /// assert!(TextForgeErrorCode::IndexOutOfRange("".into()).ignorable());
    /// assert!(!TextForgeErrorCode::LimitExceededError("".into()).ignorable());
    /// ```
    pub fn ignorable(&self) -> bool {
        match self {
            Self::LimitExceededError(_) => false,
            Self::FileNotFound(_)
            | Self::TokenNotFound(_)
            | Self::TokenArrayNotFound(_)
            | Self::FileReadingError(_)
            | Self::FileWritingError(_)
            | Self::FileOpeningError(_)
            | Self::BytecodeNotFound(_)
            | Self::BlockNotFound(_)
            | Self::VariableNotFound(_)
            | Self::NonMutableVariableError(_)
            | Self::InvalidOperands(_)
            | Self::IndexOutOfRange(_)
            | Self::InvalidIndex(_)
            | Self::InvalidParameters(_)
            | Self::InvalidArgumentNumber(_)
            | Self::BytecodeParamNotRecognized(_)
            | Self::TextParsingError(_)
            | Self::BytecodeParsingError(_)
            | Self::BytecodeParamParsingError(_)
            | Self::ValidationError(_)
            | Self::ZeroDivisionError(_)
            | Self::TryIntoFailError(_)
            | Self::IncompatibleTypeError(_)
            | Self::RequiredContextError(_)
            | Self::WatcherNotFoundError(_)
            | Self::SerializationError(_)
            | Self::GenericError(_)
            | Self::NestedBlocksNotAllowedError(_)
            | Self::InvalidUtf8Error(_)
            | Self::InvalidExprError(_) => true,
        }
    }

    /// A short suggestion for this kind of error, when there is a general one
    /// (reports print it as `Hint:`).
    ///
    /// ```
    /// use textforge::utils::errors::TextForgeErrorCode;
    /// assert!(TextForgeErrorCode::IndexOutOfRange("".into()).hint().unwrap().contains("0"));
    /// ```
    pub fn hint(&self) -> Option<&'static str> {
        Some(match self {
            Self::IndexOutOfRange(_) | Self::InvalidIndex(_) => {
                "indices count characters, words and lines from 0; the last character is __LEN_CHARS__ - 1 and the last line __LINES__ - 1 (see docs/indices-and-ranges.md)"
            }
            Self::VariableNotFound(_) => {
                "define the variable with `var` or `val` before the instruction that reads it"
            }
            Self::NonMutableVariableError(_) => {
                "a variable defined with `val` cannot change; define it with `var` to change it with `mutv` or `eval`"
            }
            Self::BlockNotFound(_) => {
                "define the block with `blk {name} assoc {instruction};` before `cblk`"
            }
            Self::TokenNotFound(_) => {
                "the instruction index lists every instruction (README.md or the `tokens` page on docs.rs)"
            }
            Self::FileNotFound(_) | Self::FileOpeningError(_) => {
                "relative paths start from the directory of the pipeline file (in code, from the working directory)"
            }
            Self::LimitExceededError(_) => {
                "raise the limit or set it to \"unlimited\": .txfrc overrides, the txf flag or ExecutionProfile::limits; tryi does not ignore limits"
            }
            Self::InvalidExprError(_) => {
                "an expression in [[ ]] or eval must produce an integer or text"
            }
            Self::NestedBlocksNotAllowedError(_) => {
                "define each block at the top level of the pipeline"
            }
            Self::WatcherNotFoundError(_) => "`txf --help` lists the built-in watchers",
            _ => return None,
        })
    }

    /// Kept name, but now returns borrowed data (no allocation).
    pub fn get_message(&self) -> &Cow<'static, str> {
        self.message()
    }

    /// Zero-allocation access to the inner message.
    pub fn message(&self) -> &Cow<'static, str> {
        match self {
            Self::InvalidExprError(x)
            | Self::InvalidUtf8Error(x)
            | Self::FileNotFound(x)
            | Self::IndexOutOfRange(x)
            | Self::InvalidIndex(x)
            | Self::InvalidOperands(x)
            | Self::TextParsingError(x)
            | Self::TokenNotFound(x)
            | Self::BytecodeNotFound(x)
            | Self::BytecodeParsingError(x)
            | Self::InvalidParameters(x)
            | Self::ValidationError(x)
            | Self::InvalidArgumentNumber(x)
            | Self::FileReadingError(x)
            | Self::FileWritingError(x)
            | Self::FileOpeningError(x)
            | Self::BytecodeParamParsingError(x)
            | Self::ZeroDivisionError(x)
            | Self::TokenArrayNotFound(x)
            | Self::BlockNotFound(x)
            | Self::VariableNotFound(x)
            | Self::NonMutableVariableError(x)
            | Self::TryIntoFailError(x)
            | Self::IncompatibleTypeError(x)
            | Self::BytecodeParamNotRecognized(x)
            | Self::WatcherNotFoundError(x)
            | Self::SerializationError(x)
            | Self::GenericError(x)
            | Self::NestedBlocksNotAllowedError(x)
            | Self::LimitExceededError(x)
            | Self::RequiredContextError(x) => x,
        }
    }

    pub(crate) fn severity_color(&self) -> Color {
        match self {
            // "Hard" errors
            Self::InvalidExprError(_)
            | Self::InvalidUtf8Error(_)
            | Self::ZeroDivisionError(_)
            | Self::IncompatibleTypeError(_)
            | Self::InvalidOperands(_)
            | Self::ValidationError(_)
            | Self::InvalidParameters(_)
            | Self::InvalidArgumentNumber(_)
            | Self::BytecodeParsingError(_)
            | Self::BytecodeParamParsingError(_)
            | Self::WatcherNotFoundError(_)
            | Self::SerializationError(_)
            | Self::GenericError(_)
            | Self::NestedBlocksNotAllowedError(_)
            | Self::LimitExceededError(_)
            | Self::TextParsingError(_) => Color::Red,
            // "Missing things" / lookup failures
            Self::TryIntoFailError(_)
            | Self::FileNotFound(_)
            | Self::FileOpeningError(_)
            | Self::FileReadingError(_)
            | Self::FileWritingError(_)
            | Self::TokenNotFound(_)
            | Self::TokenArrayNotFound(_)
            | Self::BlockNotFound(_)
            | Self::VariableNotFound(_)
            | Self::NonMutableVariableError(_)
            | Self::BytecodeNotFound(_)
            | Self::RequiredContextError(_)
            | Self::BytecodeParamNotRecognized(_) => Color::Yellow,

            // Index-ish
            Self::InvalidIndex(_) | Self::IndexOutOfRange(_) => Color::Magenta,
        }
    }
}

/// The error for an unknown pipeline identifier, built when the closure is called.
pub fn token_array_not_found(identifier: &str) -> impl Fn() -> TextForgeError {
    // Built only when called: lookups that succeed format nothing.
    move || {
        TextForgeError::new(
            TextForgeErrorCode::TokenArrayNotFound(
                format!(
                    "no pipeline is registered with the id `{identifier}`; use the id returned when the pipeline was registered"
                )
                .into(),
            ),
            Cow::Borrowed("processor"),
            Cow::Owned(identifier.to_owned()),
        )
    }
}

#[cfg(feature = "test_access")]
#[cfg(test)]
#[allow(deprecated)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn disable_colors() {
        // Keeps the tests deterministic (no ANSI).
        colored::control::set_override(false);
    }

    // ============================================================
    // TextForgeError
    // ============================================================

    #[test]
    fn textforge_error_new_sets_fields_and_helpers_return_strs() {
        disable_colors();

        let err = TextForgeError::new(
            TextForgeErrorCode::ValidationError(Cow::Borrowed("bad params")),
            Cow::Borrowed("raw"),
            Cow::Borrowed("banana"),
        );

        assert_eq!(err.instruction, Cow::Borrowed("raw"));
        assert_eq!(err.input, Cow::Borrowed("banana"));

        assert_eq!(err.instruction_str(), "raw");
        assert_eq!(err.input_str(), "banana");
    }

    #[test]
    fn textforge_error_display_contains_sections_and_no_weird_comma() {
        disable_colors();

        let err = TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(Cow::Borrowed("invalid index")),
            "dlc",
            "banana",
        );

        assert_eq!(
            format!("{err}"),
            "error[13] InvalidIndex: invalid index (in: dlc)"
        );
    }

    #[test]
    fn display_omits_empty_sections() {
        disable_colors();
        let err = TextForgeError::new(TextForgeErrorCode::GenericError("oops".into()), "", "");
        assert_eq!(format!("{err}"), "error[27] GenericError: oops");
    }

    // ============================================================
    // TextForgeErrorCode (message/get_message, Display, get_error_code, severity_color)
    // ============================================================

    #[test]
    fn error_code_message_and_get_message_match_and_are_borrowed() {
        let code = TextForgeErrorCode::TokenNotFound(Cow::Borrowed("no token"));
        assert_eq!(code.message().as_ref(), "no token");
        assert_eq!(code.get_message().as_ref(), "no token");
    }

    #[test]
    fn error_code_display_includes_labels_code_and_message() {
        disable_colors();

        let code = TextForgeErrorCode::FileNotFound(Cow::Borrowed("missing file"));
        let s = format!("{code}");

        assert_eq!(s, "error[1] FileNotFound: missing file");
    }

    #[test]
    fn get_error_code_matches_all_variants() {
        use TextForgeErrorCode::*;

        // covers every arm of get_error_code()
        assert_eq!(FileNotFound(Cow::Borrowed("x")).get_error_code(), 1);
        assert_eq!(TokenNotFound(Cow::Borrowed("x")).get_error_code(), 2);
        assert_eq!(TokenArrayNotFound(Cow::Borrowed("x")).get_error_code(), 3);
        assert_eq!(FileReadingError(Cow::Borrowed("x")).get_error_code(), 4);
        assert_eq!(FileWritingError(Cow::Borrowed("x")).get_error_code(), 5);
        assert_eq!(FileOpeningError(Cow::Borrowed("x")).get_error_code(), 6);
        assert_eq!(BytecodeNotFound(Cow::Borrowed("x")).get_error_code(), 7);
        assert_eq!(BlockNotFound(Cow::Borrowed("x")).get_error_code(), 8);
        assert_eq!(VariableNotFound(Cow::Borrowed("x")).get_error_code(), 9);
        assert_eq!(
            NonMutableVariableError(Cow::Borrowed("x")).get_error_code(),
            10
        );
        assert_eq!(InvalidOperands(Cow::Borrowed("x")).get_error_code(), 11);
        assert_eq!(IndexOutOfRange(Cow::Borrowed("x")).get_error_code(), 12);
        assert_eq!(InvalidIndex(Cow::Borrowed("x")).get_error_code(), 13);
        assert_eq!(InvalidParameters(Cow::Borrowed("x")).get_error_code(), 14);
        assert_eq!(
            InvalidArgumentNumber(Cow::Borrowed("x")).get_error_code(),
            15
        );
        assert_eq!(
            BytecodeParamNotRecognized(Cow::Borrowed("x")).get_error_code(),
            16
        );
        assert_eq!(TextParsingError(Cow::Borrowed("x")).get_error_code(), 17);
        assert_eq!(
            BytecodeParsingError(Cow::Borrowed("x")).get_error_code(),
            18
        );
        assert_eq!(
            BytecodeParamParsingError(Cow::Borrowed("x")).get_error_code(),
            19
        );
        assert_eq!(ValidationError(Cow::Borrowed("x")).get_error_code(), 20);
        assert_eq!(ZeroDivisionError(Cow::Borrowed("x")).get_error_code(), 21);
        assert_eq!(TryIntoFailError(Cow::Borrowed("x")).get_error_code(), 22);
        assert_eq!(
            IncompatibleTypeError(Cow::Borrowed("x")).get_error_code(),
            23
        );
        assert_eq!(
            RequiredContextError(Cow::Borrowed("x")).get_error_code(),
            24
        );
    }

    #[test]
    fn severity_color_red_for_hard_errors() {
        use TextForgeErrorCode::*;

        assert_eq!(
            ZeroDivisionError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidOperands(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            ValidationError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidParameters(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidArgumentNumber(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            BytecodeParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            BytecodeParamParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            TextParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
    }

    #[test]
    fn severity_color_yellow_for_missing_things() {
        use TextForgeErrorCode::*;

        assert_eq!(
            FileNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileOpeningError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileReadingError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileWritingError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            TokenNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            TokenArrayNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BlockNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            VariableNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            NonMutableVariableError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BytecodeNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BytecodeParamNotRecognized(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
    }

    #[test]
    fn severity_color_magenta_for_indexish() {
        use TextForgeErrorCode::*;

        assert_eq!(
            InvalidIndex(Cow::Borrowed("x")).severity_color(),
            Color::Magenta
        );
        assert_eq!(
            IndexOutOfRange(Cow::Borrowed("x")).severity_color(),
            Color::Magenta
        );
    }

    #[test]
    fn message_returns_the_same_inner_reference_for_all_variants() {
        // cobre o match "achatado" (| A(x) | B(x) => x) do method message()
        // with a few examples from different groups.
        use TextForgeErrorCode::*;

        let m1 = FileNotFound(Cow::Borrowed("a")).message().as_ref();
        let m2 = InvalidOperands(Cow::Borrowed("b")).message().as_ref();
        let m3 = IndexOutOfRange(Cow::Borrowed("c")).message().as_ref();
        let m4 = BytecodeParamNotRecognized(Cow::Borrowed("d"))
            .message()
            .as_ref();

        assert_eq!(m1, "a");
        assert_eq!(m2, "b");
        assert_eq!(m3, "c");
        assert_eq!(m4, "d");
    }

    // ============================================================
    // ErrorManager (add_error/has_errors/reserve_errors/will_log/will_panic/write_errors/handle_error)
    // ============================================================

    #[test]
    fn error_manager_default_and_add_error_has_errors() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        assert!(!mgr.has_errors());

        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::TokenNotFound(Cow::Borrowed("x")),
            "inst",
            "in",
        ));

        assert!(mgr.has_errors());
        assert_eq!(mgr.error_vec.len(), 1);
    }

    #[test]
    fn error_manager_reserve_errors_does_not_decrease_capacity() {
        let mut mgr = ErrorManager::default();

        let before = mgr.error_vec.capacity();
        mgr.reserve_errors(256);
        let after = mgr.error_vec.capacity();

        assert!(after >= before, "reserve must not reduce the capacity");
    }

    #[test]
    fn will_log_toggles_show_warnings_and_does_not_panic() {
        let mut mgr = ErrorManager::default();

        mgr.will_log(true);
        assert!(mgr.show_warnings);

        mgr.will_log(false);
        assert!(!mgr.show_warnings);
    }

    #[test]
    fn will_panic_toggles_panic_mode_flag() {
        let mut mgr = ErrorManager::default();

        mgr.will_panic(true);
        assert!(mgr.panic_with_error);

        mgr.will_panic(false);
        assert!(!mgr.panic_with_error);
    }

    #[test]
    fn write_errors_writes_expected_sections_and_indices_for_multiple_errors() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::TokenNotFound(Cow::Borrowed("first")),
            "inst1",
            "in1",
        ));
        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(Cow::Borrowed("second")),
            "inst2",
            "in2",
        ));

        let mut buf: Vec<u8> = vec![];
        mgr.write_errors(&mut buf).unwrap();

        let out = String::from_utf8(buf).unwrap();

        // sections of the writer helper
        assert!(out.contains("Code:"));
        assert!(out.contains("Instruction:"));
        assert!(out.contains("Message:"));
        assert!(out.contains("Processing:"));

        // enumerated indices
        assert!(out.contains("[0]"));
        assert!(out.contains("[1]"));

        // message, instruction and input contents
        assert!(out.contains("first"));
        assert!(out.contains("second"));
        assert!(out.contains("inst1"));
        assert!(out.contains("inst2"));
        assert!(out.contains("in1"));
        assert!(out.contains("in2"));
    }

    #[test]
    fn handle_error_stores_error_and_does_not_panic_when_panic_mode_off() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.will_log(false);
        mgr.will_panic(false);

        mgr.handle_error(TextForgeError::new(
            TextForgeErrorCode::ValidationError(Cow::Borrowed("oops")),
            "inst",
            "input",
        ));

        assert!(mgr.has_errors());
        assert_eq!(mgr.error_vec.len(), 1);
        assert_eq!(mgr.error_vec[0].instruction_str(), "inst");
        assert_eq!(mgr.error_vec[0].input_str(), "input");
    }

    #[test]
    fn handle_error_panics_when_panic_mode_on_and_still_stores_error_first() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.will_log(false);
        mgr.will_panic(true);

        let err = TextForgeError::new(
            TextForgeErrorCode::ZeroDivisionError(Cow::Borrowed("div0")),
            "div",
            "10/0",
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mgr.handle_error(err);
        }));

        assert!(result.is_err(), "must panic when panic_with_error=true");
        assert!(
            mgr.has_errors(),
            "must have stored the error before panicking"
        );
        assert_eq!(mgr.error_vec.len(), 1);
    }

    // ============================================================
    // token_array_not_found
    // ============================================================

    #[test]
    fn token_array_not_found_closure_returns_stable_clone_and_contains_identifier() {
        disable_colors();

        let f = token_array_not_found("abc");

        let e1 = f();
        let e2 = f();

        assert_eq!(e1, e2);
        assert_eq!(e1.instruction, Cow::Borrowed("processor"));
        assert_eq!(e1.input, Cow::Borrowed("abc"));

        match &e1.error_code {
            TextForgeErrorCode::TokenArrayNotFound(m) => {
                assert!(m.contains("no pipeline is registered"));
                assert!(m.contains("abc"));
            }
            _ => panic!("expected TokenArrayNotFound"),
        }
    }
}
