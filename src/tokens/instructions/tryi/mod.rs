#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    parse_args,
    parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

/// `tryi {instruction};` — Run `instruction`; if it fails, skip it (text and variables unchanged) and continue.
///
/// When `instruction` succeeds, `tryi` is the same as `instruction`. When it
/// fails, the pipeline continues as if it had never run: the text is the text
/// `tryi` received, and variables, blocks, replacement maps and `end` are put
/// back as they were. Only two things stay: the safety limits are never
/// ignored (a `LimitExceededError` stops the pipeline, as without `tryi`), and
/// a file `wtf` already wrote stays written (writing it again replaces it).
///
/// The ignored error is not lost: the debug methods (`process_all_with_debug`,
/// `txf --debug`) show it where it happened, as a warning with its trace and
/// parameters, and list every ignored error at the end. See
/// `docs/error-handling.md`.
///
/// ```rust
/// use textforge::api::{processor::{TextForgeProcessor, TextForgeProcessorMethods}, TextForgeBuilderMethods};
/// use textforge::tokens::{InstructionMethods, transforms::dlc::Dlc};
/// let mut processor = TextForgeProcessor::new();
/// let delete: Box<dyn InstructionMethods> = Box::new(Dlc::new(5, 9)?);
/// let id = processor.create_pipeline().try_ignore(delete)?.add_to_end("!")?.build()?;
/// assert_eq!(processor.process_all(&id, "abcdefghij")?, "abcde!"); // dlc ran
/// assert_eq!(processor.process_all(&id, "abc")?, "abc!"); // dlc failed: skipped
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone, Default)]
pub struct Tryi {
    inner: TokenWrapper,
    params: Vec<TextForgeParamTypes>,
}

impl Tryi {
    /// Builds `tryi {instruction};`.
    pub fn new(inner: TokenWrapper) -> Self {
        Tryi {
            params: vec![TextForgeParamTypes::Token(inner.clone())],
            inner,
        }
    }

    /// Whether the instruction runs others, so it can change the context
    /// before it fails (a block, a loop, a section, a condition, a `tryi`).
    /// The others either fail before they change anything or change nothing
    /// but the text, so they need no snapshot.
    fn runs_others(&self) -> bool {
        matches!(
            self.inner.get_string_repr(),
            "cblk" | "iter" | "ifdc" | "onln" | "onck" | "onlni" | "tryi"
        )
    }
}

impl InstructionMethods for Tryi {
    /// Holds only immutable bound state; the nested instruction keeps its own
    /// (possibly dynamic) parameters.
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x5a
    }

    fn get_string_repr(&self) -> &'static str {
        "tryi"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("tryi {}", self.inner.to_textforge_line()).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut own_context;
        let context = match context {
            Some(context) => context,
            None => {
                own_context = GlobalExecutionContext::new();
                &mut own_context
            }
        };
        let snapshot = self.runs_others().then(|| context.snapshot());
        let depth = context.trace_mut().map(|trace| trace.depth());

        // The instruction borrows the text, so it is still here if it fails.
        // A result that is the whole text is the text itself (no copy).
        let outcome = match &input {
            Cow::Borrowed(text) => self
                .inner
                .apply_nested(Cow::Borrowed(*text), context)
                .map(Some),
            Cow::Owned(text) => self
                .inner
                .apply_nested(Cow::Borrowed(text.as_str()), context)
                .map(|result| match result {
                    Cow::Borrowed(result) if result.len() == text.len() => None,
                    result => Some(Cow::Owned(result.into_owned())),
                }),
        };
        match outcome {
            Ok(Some(result)) => Ok(result),
            Ok(None) => Ok(input),
            Err(error) if error.is_ignorable() => {
                let restored = snapshot
                    .map(|snapshot| context.restore(snapshot))
                    .unwrap_or_default();
                if let (Some(depth), Some(trace)) = (depth, context.trace_mut()) {
                    trace.ignore(depth, &error, &restored);
                }
                Ok(input)
            }
            Err(error) => Err(error),
        }
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 1, "tryi", "")?;
        self.inner = parse_args!(params, 0, Token, "instruction");
        self.params = vec![TextForgeParamTypes::Token(self.inner.clone())];
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        Ok(to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::Token(self.inner.clone())]
        ))
    }
}
