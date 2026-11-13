//! Read-only facts about the pipeline being run (see `docs/execution-context.md`).
//!
//! A [`StaticContext`] is built by the processor for each run and shared through
//! an [`Arc`]. It never changes while the pipeline runs, so concurrent batch tasks
//! can share it, and it never depends on the text being processed.
use std::{
    path::{Path, PathBuf},
    sync::{Arc, LazyLock},
};

/// Registry of global functions available to `[[ ]]` expressions.
///
/// Empty in this version: it exists so that functions can later be added to
/// expression evaluation (`utils::expr`) without changing the execution engine.
/// There is no public way to register functions yet.
///
/// Expressions see the registered functions in addition to evalexpr's own:
///
/// ```
/// use textforge::api::TextForgeBuilderMethods;
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().add_to_end("[[max(2, 7)]]")?.build()?;
/// assert_eq!(processor.process_all(&id, "max: ")?, "max: 7");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Debug, Default, Clone)]
pub struct GlobalFunctions {
    _private: (),
}

impl GlobalFunctions {
    /// Whether no function is registered (always true in this version).
    pub fn is_empty(&self) -> bool {
        true
    }

    /// Add the registered functions to an expression context. Nothing to add
    /// yet; this is the single point where functions will enter evaluation.
    pub(crate) fn install(&self, _context: &mut evalexpr::HashMapContext) {}
}

/// Immutable facts about a pipeline run, readable by the DSL through built-in
/// variables (`__PIPELINE_NAME__`, `__TEXTFORGE_VERSION__`, `__INPUT_FILE__`).
///
/// Users never build one: the processor fills it from the registered pipeline
/// (its name) and the processing method (the input file, when there is one).
///
/// ```
/// use textforge::api::TextForgeBuilderMethods;
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().add_to_end("{{__PIPELINE_NAME__}}")?.build()?;
/// processor.set_pipeline_name(&id, "report")?;
/// assert_eq!(processor.process_all(&id, "name: ")?, "name: report");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Debug, Clone)]
pub struct StaticContext {
    pipeline_name: String,
    textforge_version: &'static str,
    input_file: Option<PathBuf>,
    functions: GlobalFunctions,
}

static DEFAULT: LazyLock<Arc<StaticContext>> =
    LazyLock::new(|| Arc::new(StaticContext::new(String::new(), None)));

impl StaticContext {
    pub(crate) fn new(pipeline_name: String, input_file: Option<PathBuf>) -> Self {
        StaticContext {
            pipeline_name,
            textforge_version: env!("CARGO_PKG_VERSION"),
            input_file,
            functions: GlobalFunctions::default(),
        }
    }

    /// The same facts for a run reading `input_file`.
    pub(crate) fn with_input_file(&self, input_file: &Path) -> Self {
        StaticContext {
            input_file: Some(input_file.to_path_buf()),
            ..self.clone()
        }
    }

    /// The context used when no pipeline is involved (a single instruction,
    /// or a context created directly): empty name and no input file.
    pub(crate) fn shared_default() -> Arc<StaticContext> {
        Arc::clone(&DEFAULT)
    }

    /// Name of the running pipeline.
    pub fn pipeline_name(&self) -> &str {
        &self.pipeline_name
    }

    /// Version of the TextForge core running the pipeline.
    pub fn textforge_version(&self) -> &'static str {
        self.textforge_version
    }

    /// The input file, when the run reads one (`process_file`, `process_batch`).
    pub fn input_file(&self) -> Option<&Path> {
        self.input_file.as_deref()
    }

    /// Global functions available to expressions.
    pub fn functions(&self) -> &GlobalFunctions {
        &self.functions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_context_is_shareable_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StaticContext>();
        assert_send_sync::<Arc<StaticContext>>();
    }

    #[test]
    fn default_has_no_name_and_no_input_file() {
        let context = StaticContext::shared_default();
        assert_eq!(context.pipeline_name(), "");
        assert_eq!(context.input_file(), None);
        assert_eq!(context.textforge_version(), env!("CARGO_PKG_VERSION"));
        assert!(context.functions().is_empty());
    }
}
