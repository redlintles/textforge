#[cfg(feature = "test_access")]
pub mod test;

use std::{borrow::Cow, path::Path, sync::Arc};

use crate::{
    context::{debug_trace::within, execution_context::GlobalExecutionContext},
    parser::{
        joins::{self, JoinState, JoinedPipeline},
        params::TextForgeParamTypes,
    },
    tokens::InstructionMethods,
    utils::{
        apply::run_tokens,
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

/// Ppjn - Pipeline Join
///
/// Runs a whole pipeline from another file on the current text and continues
/// with its result, like a subshell: text in, text out. The joined pipeline
/// runs in a **new** execution context: it cannot see the outer variables,
/// blocks or replacement maps, and its own do not leak out. Only the read-only
/// static context is shared (`__PIPELINE_NAME__` stays the outer name). `end`
/// inside it ends only the joined pipeline.
///
/// The path is a literal, resolved against the directory of the file that
/// contains `ppjn` (or the working directory). The file, text or bytecode
/// (recognized by its first bytes), is loaded once with the outer pipeline,
/// never while it runs; cycles and nesting deeper than
/// [`MAX_JOIN_DEPTH`](crate::parser::joins::MAX_JOIN_DEPTH) are errors.
/// Bytecode embeds the joined pipeline. See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::api::{processor::{TextForgeProcessor, TextForgeProcessorMethods}, TextForgeBuilderMethods};
///
/// let directory = std::env::temp_dir().join(format!("ppjn-doc-{}", std::process::id()));
/// std::fs::create_dir_all(&directory).unwrap();
/// let library = directory.join("clean.textforge");
/// std::fs::write(&library, "tbs;\nvar inner = 1;\nate '!';\n").unwrap();
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline()
///     .pipeline_join(library.to_string_lossy().as_ref())?
///     .add_to_beginning("> ")?
///     .build()?;
/// assert_eq!(processor.process_all(&id, "  hi  ")?, "> hi!");
/// # std::fs::remove_dir_all(&directory).unwrap();
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone)]
pub struct Ppjn {
    path: String,
    pipeline: JoinState,
    params: Vec<TextForgeParamTypes>,
}

impl Default for Ppjn {
    fn default() -> Self {
        Ppjn {
            path: String::new(),
            pipeline: JoinState::Unloaded,
            params: vec![
                TextForgeParamTypes::String(String::new()),
                TextForgeParamTypes::Pipeline(JoinState::Unloaded),
            ],
        }
    }
}

impl Ppjn {
    /// The joined pipeline; loaded now (from the working directory) only when
    /// the outer pipeline was not loaded through a path that loads joins.
    fn joined(&self) -> Result<Arc<JoinedPipeline>, TextForgeError> {
        match &self.pipeline {
            JoinState::Loaded(pipeline) => Ok(Arc::clone(pipeline)),
            JoinState::Unloaded => joins::load(&self.path, Path::new("."), &[]),
        }
    }

    fn origin<'a>(&'a self, pipeline: &'a JoinedPipeline) -> &'a str {
        if pipeline.origin().is_empty() {
            &self.path
        } else {
            pipeline.origin()
        }
    }
}

/// A fresh context sharing only the outer static context, and the debug trace
/// while the joined pipeline runs.
fn joined_context(outer: &mut Option<&mut GlobalExecutionContext>) -> GlobalExecutionContext {
    let mut inner = match outer {
        Some(outer) => GlobalExecutionContext::with_static(outer.shared_static()),
        None => GlobalExecutionContext::new(),
    };
    if let Some(trace) = outer.as_mut().and_then(|outer| outer.take_trace()) {
        inner.put_trace(trace);
    }
    inner
}

fn give_back_trace(inner: &mut GlobalExecutionContext, outer: Option<&mut GlobalExecutionContext>) {
    if let (Some(trace), Some(outer)) = (inner.take_trace(), outer) {
        outer.put_trace(trace);
    }
}

impl InstructionMethods for Ppjn {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "ppjn"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ppjn {};\n", self.path).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        mut context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let pipeline = self.joined()?;
        let origin = self.origin(&pipeline);
        let mut inner = joined_context(&mut context);
        let result = within(
            &mut inner,
            |_| format!("ppjn {origin}  ({} instructions)", pipeline.tokens().len()),
            origin,
            input,
            |input, inner| {
                run_tokens(
                    pipeline.tokens(),
                    input,
                    inner,
                    |index, token, text, inner| {
                        token
                            .apply_nested(text, inner)
                            .map_err(|error| joins::in_join(error, origin, pipeline.line(index)))
                    },
                )
            },
        );
        give_back_trace(&mut inner, context);
        result
    }

    /// Runs the joined pipeline in buffered mode too, without materializing.
    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        mut context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        if context.as_ref().is_some_and(|context| context.is_tracing()) {
            // Debug output describes nested steps on materialized text.
            return input.transform_with(|text| self.transform(text, context));
        }
        let pipeline = self.joined()?;
        let origin = self.origin(&pipeline);
        let mut inner = joined_context(&mut context);
        run_tokens(
            pipeline.tokens(),
            input,
            &mut inner,
            |index, token, text, inner| {
                token
                    .apply_buffered(text, inner)
                    .map_err(|error| joins::in_join(error, origin, pipeline.line(index)))
            },
        )
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        check_vec_len(params, 2, "ppjn", "")?;
        let (TextForgeParamTypes::String(path), TextForgeParamTypes::Pipeline(pipeline)) =
            (&params[0], &params[1])
        else {
            return Err(TextForgeError::new(
                TextForgeErrorCode::InvalidParameters("ppjn takes a path".into()),
                "ppjn",
                "",
            ));
        };
        self.path = path.clone();
        self.pipeline = pipeline.clone();
        self.params = params.to_vec();
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x58
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        Ok(to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::String(self.path.clone()),
                TextForgeParamTypes::Pipeline(self.pipeline.clone()),
            ]
        ))
    }
}
