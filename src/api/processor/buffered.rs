//! Opt-in whole-document execution; existing String-returning APIs are unchanged.
use std::path::{Path, PathBuf};

use super::TextForgeProcessor;
use crate::api::{TextBuffer, TextBufferOptions};
use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::{TextForgeError, token_array_not_found};

/// An owned batch input descriptor suitable for wrapping behind future FFI handles.
/// Results keep descriptor order. The descriptor contains no Rust token objects.
///
/// ```
/// use textforge::api::processor::BufferedTask;
/// let task = BufferedTask::new("registered-pipeline-id", "input.txt");
/// assert_eq!(task.pipeline_id(), "registered-pipeline-id");
/// ```
#[derive(Debug)]
pub struct BufferedTask {
    pipeline_id: String,
    input_path: PathBuf,
}

impl BufferedTask {
    /// Own a pipeline identifier and input path for later execution.
    pub fn new(pipeline_id: impl Into<String>, input_path: impl Into<PathBuf>) -> Self {
        Self {
            pipeline_id: pipeline_id.into(),
            input_path: input_path.into(),
        }
    }
    /// Registered pipeline identifier.
    pub fn pipeline_id(&self) -> &str {
        &self.pipeline_id
    }
    /// Input document path.
    pub fn input_path(&self) -> &Path {
        &self.input_path
    }
}

impl TextForgeProcessor {
    fn run_buffered(&self, id: &str, mut input: TextBuffer) -> Result<TextBuffer, TextForgeError> {
        let tokens = self
            .transforms
            .get(id)
            .ok_or_else(token_array_not_found(id))?;
        let mut context = GlobalExecutionContext::new();
        for token in tokens {
            if context.is_halted {
                break;
            }
            input = token.apply_buffered(input, &mut context)?;
        }
        Ok(input)
    }

    /// Execute one complete logical document with one execution context.
    /// Prefix, suffix, repetition, null and end use native buffered operations;
    /// other instructions materialize through the existing transform implementation.
    ///
    /// ```
    /// use textforge::api::{TextBuffer, TextBufferOptions, TextForgeBuilderMethods};
    /// use textforge::api::processor::TextForgeProcessor;
    /// let mut processor = TextForgeProcessor::new();
    /// let id = processor.create_pipeline().add_to_beginning("<")?.add_to_end(">")?.build();
    /// let input = TextBuffer::from_text("👩🏽‍💻", TextBufferOptions::new(1, std::env::temp_dir()))?;
    /// let output = processor.process_all_buffered(&id, input)?;
    /// assert!(output.is_spilled());
    /// assert_eq!(output.into_string()?, "<👩🏽‍💻>");
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn process_all_buffered(
        &mut self,
        id: &str,
        input: TextBuffer,
    ) -> Result<TextBuffer, TextForgeError> {
        self.run_buffered(id, input)
            .inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Parse and execute one textual instruction against a single owned document.
    /// This avoids exposing parser/token types in the new src/api entry point.
    pub fn process_single_buffered(
        &mut self,
        instruction: &str,
        input: TextBuffer,
    ) -> Result<TextBuffer, TextForgeError> {
        let result = crate::text::reader::read_from_text(instruction)
            .and_then(|token| token.apply_buffered(input, &mut GlobalExecutionContext::new()));
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Read a file with bounded UTF-8 validation and process it as one document.
    /// No mmap is used by this execution path. Output conversion/writing is explicit.
    pub fn process_file_buffered(
        &mut self,
        id: &str,
        path: impl AsRef<Path>,
        options: TextBufferOptions,
    ) -> Result<TextBuffer, TextForgeError> {
        let result =
            TextBuffer::from_file(path, options).and_then(|input| self.run_buffered(id, input));
        result.inspect_err(|error| self.errors.add_error(error.clone()))
    }

    /// Execute independent documents sequentially, preserving result order and
    /// continuing after errors. Completed results are forced to disk so retaining
    /// the result vector does not retain one in-memory spool per successful task.
    /// This is deliberately a conservative experiment, not the legacy parallel scheduler.
    /// File descriptors, disk space and fallback allocations are not globally capped.
    ///
    /// ```
    /// use textforge::api::{TextBufferOptions, TextForgeBuilderMethods};
    /// use textforge::api::processor::TextForgeProcessor;
    /// let mut processor = TextForgeProcessor::new();
    /// let _id = processor.create_pipeline().add_to_end("!")?.build();
    /// let results = processor.process_batch_buffered(&[], &TextBufferOptions::new(1024, std::env::temp_dir()));
    /// assert!(results.is_empty());
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn process_batch_buffered(
        &self,
        tasks: &[BufferedTask],
        options: &TextBufferOptions,
    ) -> Vec<Result<TextBuffer, TextForgeError>> {
        tasks
            .iter()
            .map(|task| {
                TextBuffer::from_file(&task.input_path, options.clone())
                    .and_then(|input| self.run_buffered(&task.pipeline_id, input))
                    .and_then(|mut output| {
                        output.spill()?;
                        Ok(output)
                    })
            })
            .collect()
    }
}
