use crate::utils::errors::{ErrorParameter, TextForgeError, TraceFrame};
use std::{error::Error, fmt};

/// The operation that failed at the portable API boundary.
///
/// Conversion checks names/numeric representation, validation checks the complete
/// hierarchy, and execution resolves variables and transforms input. See
/// [`super::PipelinePlan::compile`] and [`super::CompiledPipeline::process`].
///
/// ```
/// use textforge::api::portable::{ErrorStage, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// let error = plan.add(InstructionSpec::new("unknown".into(), vec![])).unwrap_err();
/// assert_eq!(error.stage, ErrorStage::Conversion);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorStage {
    /// Input could not be represented as an instruction or native-size number.
    Conversion,
    /// The complete construction hierarchy violates a structural rule.
    Validation,
    /// A validated pipeline failed while processing a particular input.
    Execution,
    /// Pipeline import/export or report persistence failed.
    Persistence,
    /// A user-provided observer returned an error.
    Callback,
}

/// Owned, uncolored error data suitable for exceptions or foreign result objects.
///
/// `code` preserves the engine's numeric error code; `message`, `instruction`,
/// and `input` preserve its raw context; [`details`](BindingErrorDetails) carry
/// the error's kind, trace, parameters, cause and hint (0.4.0), and the plain
/// report a person reads. Adapters should read fields rather than parse
/// [`fmt::Display`] or `report`. No borrowed engine object escapes this
/// boundary.
/// See [`ErrorStage`] and [`super::CompiledPipeline::process`].
///
/// ```
/// use textforge::api::portable::{ErrorStage, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// let error = plan.add(InstructionSpec::new("unknown".into(), vec![])).unwrap_err();
/// assert_eq!(error.code, 2);
/// assert_eq!(error.stage, ErrorStage::Conversion);
/// assert_eq!(error.details.kind, "TokenNotFound");
/// assert!(!error.message.contains('\u{1b}'));
/// assert!(error.details.report.starts_with("error[2] TokenNotFound: "));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingError {
    /// Boundary that returned this error, including persistence and callbacks.
    pub stage: ErrorStage,
    /// Existing engine error code, without remapping into language-specific codes.
    pub code: u16,
    /// Raw message; no terminal color formatting or presentation labels are added.
    pub message: String,
    /// Engine instruction context, when available.
    pub instruction: String,
    /// Engine input/diagnostic context, when available.
    pub input: String,
    /// Kind, trace, parameters, cause, hint and report (boxed: the error
    /// stays small to return).
    pub details: Box<BindingErrorDetails>,
}

/// What a [`BindingError`] knows beyond its code and message (0.4.0); see
/// `docs/error-handling.md`.
///
/// ```
/// use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
/// let mut plan = PipelinePlan::new();
/// plan.add(InstructionSpec::new("dls".into(), vec![Argument::Number(9)])).unwrap();
/// let error = plan.compile().unwrap().process("abc").unwrap_err();
/// assert_eq!(error.details.parameters[0].name, "index");
/// assert!(error.details.ignorable);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BindingErrorDetails {
    /// The stable name of the code (`IndexOutOfRange`).
    pub kind: String,
    /// The instructions the error left, outermost first; the last one failed.
    pub trace: Vec<TraceFrame>,
    /// The parameters of the instruction that failed, with the values it ran with.
    pub parameters: Vec<ErrorParameter>,
    /// The lower-level error behind this one (I/O, regex), when there is one.
    pub cause: Option<String>,
    /// A general suggestion for this kind of error, when there is one.
    pub hint: Option<String>,
    /// Whether `tryi` would ignore this error (only safety limits are not).
    pub ignorable: bool,
    /// The full report without color, as `txf` prints it.
    pub report: String,
}

impl BindingError {
    pub(super) fn from_core(stage: ErrorStage, error: TextForgeError) -> Self {
        Self {
            stage,
            code: error.error_code.get_error_code(),
            message: error.error_code.message().to_string(),
            details: Box::new(BindingErrorDetails {
                kind: error.error_code.name().to_owned(),
                trace: error.trace().cloned().collect(),
                parameters: error.parameters().to_vec(),
                cause: error.cause().map(str::to_owned),
                hint: error.error_code.hint().map(str::to_owned),
                ignorable: error.is_ignorable(),
                report: error.report().color(false).to_string(),
            }),
            instruction: error.instruction.into_owned(),
            input: error.input.into_owned(),
        }
    }
}

impl fmt::Display for BindingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:?}: {} (code {}, instruction: {})",
            self.stage, self.message, self.code, self.instruction
        )
    }
}

impl Error for BindingError {}
