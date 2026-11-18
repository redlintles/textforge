//! [`TextForgeProcessor`]: registers pipelines and runs them on strings,
//! files and batches, with optional debug output, watchers and buffering.
#[cfg(feature = "buffered")]
mod buffered;
pub(crate) mod file_io;
#[cfg(feature = "buffered")]
pub use buffered::BufferedTask;

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use colored::*;
#[cfg(not(feature = "buffered"))]
use rayon::prelude::*;
use uuid::Uuid;

use std::sync::Arc;

#[cfg(feature = "bytecode")]
use crate::bytecode::{reader::read_bytecode_from_file, writer::write_bytecode_to_file};
#[cfg(all(feature = "watchers", not(feature = "buffered")))]
use crate::watchers::WatcherContext;
#[cfg(feature = "watchers")]
use crate::watchers::WatcherList;

use crate::api::builder::TextForgeBuilder;
use crate::api::profile::ExecutionProfile;
#[cfg(not(feature = "buffered"))]
use crate::context::execution_context::GlobalContextMethods;
use crate::context::execution_context::GlobalExecutionContext;
use crate::context::static_context::StaticContext;
use crate::parser::resolve_var::TokenWrapper;
use crate::text::reader::read_from_file;
use crate::text::writer::write_to_file;
use crate::utils::apply::run_tokens;
use crate::utils::errors::{TextForgeError, TextForgeErrorCode, token_array_not_found};

/// Registers pipelines and runs them.
///
/// A processor holds any number of pipelines, each identified by the string
/// returned when it is registered. A pipeline is a list of instructions run in
/// order: each one receives the output of the previous one. Pipelines are
/// registered by building them in Rust ([`create_pipeline`](Self::create_pipeline)),
/// by reading a `.textforge` file
/// ([`get_pipeline_from_file`](TextForgeProcessorMethods::get_pipeline_from_file))
/// or a `.textforgebc` file (`read_from_bytecode_file`, `bytecode` feature);
/// however they were registered, they run the same way. All operations are in
/// [`TextForgeProcessorMethods`].
///
/// An [`ExecutionProfile`] ([`with_profile`](Self::with_profile)) decides the
/// memory a run may use and the limits of work-multiplying instructions; it
/// never changes results.
///
/// # Examples
///
/// Build a pipeline and run it:
///
/// ```
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::api::TextForgeBuilderMethods;
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor
///     .create_pipeline()
///     .trim_both_sides()?
///     .add_to_end("!")?
///     .build()?;
///
/// assert_eq!(processor.process_all(&id, "   banana   ")?, "banana!");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
///
/// See every step of a run (the result is the same as `process_all`; the trace
/// is printed):
///
/// ```
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::api::TextForgeBuilderMethods;
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor
///     .create_pipeline()
///     .add_to_beginning("[")?
///     .add_to_end("]")?
///     .to_uppercase_all()?
///     .build()?;
///
/// assert_eq!(processor.process_all_with_debug(&id, "banana")?, "[BANANA]");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
///
/// Run one instruction without registering a pipeline:
///
/// ```
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::tokens::{InstructionMethods, transforms::tbs::Tbs};
///
/// let mut processor = TextForgeProcessor::new();
/// let token = (Box::new(Tbs::default()) as Box<dyn InstructionMethods>).into();
/// assert_eq!(processor.process_single(token, "   banana   ")?, "banana");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
pub struct TextForgeProcessor {
    transforms: HashMap<String, RegisteredPipeline>,
    /// Engine resources; never changes results.
    #[cfg_attr(not(feature = "buffered"), allow(dead_code))]
    profile: ExecutionProfile,
}

/// A registered pipeline: its instructions and the static context shared by
/// every run that reads no input file (its name, for `__PIPELINE_NAME__`).
struct RegisteredPipeline {
    tokens: Vec<TokenWrapper>,
    statics: Arc<StaticContext>,
    /// Directory of the file the pipeline was read from; relative paths of the
    /// file instructions start there. `None` for pipelines built in code.
    base_dir: Option<Arc<Path>>,
}

/// Everything a [`TextForgeProcessor`] does: register, persist, inspect, remove
/// and run pipelines.
///
/// A registered pipeline is identified by the string returned when it is
/// registered ([`add_transform`](Self::add_transform),
/// [`get_pipeline_from_file`](Self::get_pipeline_from_file), a builder's
/// `build()` or `read_from_bytecode_file`).
///
/// ## Method groups
///
/// - **Text persistence**: save a pipeline to, or load it from, a `.textforge` file.
/// - **Registration and inspection**: register, remove, list or copy pipelines.
/// - **Processing**: the only methods that run a pipeline on text (`process_*`,
///   `run_transform`), with debug, watcher and buffered variants.
/// - **Bytecode persistence** (`bytecode` feature): the same as text
///   persistence for `.textforgebc` files.
///
/// ## Errors
///
/// Methods return [`TextForgeError`]; print one for a person with
/// [`TextForgeError::report`]. Before 0.4.0 the processor also kept a copy of
/// every error in a private `ErrorManager` that nothing read.
pub trait TextForgeProcessorMethods {
    // -----------------------------------------------------------------
    // Pipeline persistence (text format)
    // -----------------------------------------------------------------

    /// Writes a registered transform (pipeline) to a `.textforge` text file.
    ///
    /// Internally:
    /// - looks up `id` in `self.transforms`
    /// - if found, calls `write_to_file(path, tokens)`
    /// - if not found, returns `TokenArrayNotFound` (via `token_array_not_found`)
    ///
    /// # Parameters
    /// - `id`: Transform identifier previously returned by `add_transform()` / `build()`.
    /// - `path`: Destination path to write the textual representation.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - writing fails (I/O or serialization problems inside `write_to_file`)
    fn write_pipeline_to_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError>;

    /// Reads a `.textforge` text file, parses it into tokens, registers it as a new transform,
    /// and returns the newly created transform ID.
    ///
    /// Internally:
    /// - reads and parses tokens via `read_from_file(path)`
    /// - generates a new UUID
    /// - inserts the parsed vector into `self.transforms`
    ///
    /// This only registers the pipeline — it does not process any input. To run it, pass the
    /// returned ID to one of the `process_*` methods.
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    ///
    /// # Errors
    /// Returns `Err` if reading/parsing the file fails.
    fn get_pipeline_from_file(&mut self, path: &Path) -> Result<String, TextForgeError>;

    // -----------------------------------------------------------------
    // Pipeline registration & inspection
    // -----------------------------------------------------------------

    /// Registers a new transform (pipeline) directly from a token vector.
    ///
    /// This is the low-level "insert" API. Higher-level builder APIs typically call this.
    ///
    /// Internally:
    /// - generates a new UUID
    /// - inserts `(uuid -> tokens)` into `self.transforms`
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    fn add_transform(&mut self, tokens: Vec<TokenWrapper>) -> String;

    /// Removes a transform from the processor.
    ///
    /// Internally:
    /// - performs `self.transforms.remove(id)`
    /// - returns `Ok(())` if something was removed
    /// - returns `Err(TokenNotFound)` (your custom error) if the ID does not exist
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist.
    fn remove_transform(&mut self, id: &str) -> Result<(), TextForgeError>;

    /// Displays the list of registered transform IDs.
    ///
    /// Your current implementation prints:
    /// - an index counter
    /// - the UUID key
    ///
    /// # Note
    /// This is pure side-effect (stdout). It does not return the data.
    ///
    /// The trait provides a default empty body `{}` so implementors may override it.
    fn show_transforms(&self) {}

    /// Checks whether a transform with the given `id` exists.
    ///
    /// Internally: `self.transforms.contains_key(id)`
    fn transform_exists(&self, id: &str) -> bool;

    /// Returns a **cloned** copy of the token vector for a given transform `id`.
    ///
    /// This method is useful for:
    /// - inspection
    /// - exporting
    /// - composing transforms (if you later support merging)
    ///
    /// # Returns
    /// A cloned `Vec<TokenWrapper>`.
    ///
    /// # Errors
    /// Returns `Err(TokenArrayNotFound)` if the transform does not exist.
    fn get_transform_vec(&self, id: &str) -> Result<Vec<TokenWrapper>, TextForgeError>;

    /// Returns the textual `.textforge` lines for a given transform `id`.
    ///
    /// Internally:
    /// - clones the transform vector
    /// - maps each token to `token.to_textforge_line().to_string()`
    ///
    /// This is typically what you want for:
    /// - UI display
    /// - exporting to text
    /// - debugging what the pipeline "looks like"
    ///
    /// # Errors
    /// Returns `Err(TokenArrayNotFound)` if the transform does not exist.
    fn get_text_transform_vec(&self, id: &str) -> Result<Vec<String>, TextForgeError>;

    // -----------------------------------------------------------------
    // Processing — the only methods that run a pipeline against input
    // -----------------------------------------------------------------

    /// Executes a registered transform against `input` and returns the result.
    ///
    /// This is the shared low-level execution primitive behind
    /// [`process_all`](Self::process_all) and [`process_batch`](Self::process_batch): both
    /// look up `id`, then run each token in order over a fresh
    /// `GlobalExecutionContext`. It only borrows `&self`, so concurrent runs
    /// share nothing.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - any token execution fails
    fn run_transform<'a>(
        &self,
        id: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError>;

    /// Executes all tokens of a registered transform from left to right.
    ///
    /// Semantics:
    /// - `result` starts as `input`
    /// - for each token `t` in the transform:
    ///   - `result = t.apply_token(result, context)?`
    /// - returns the final `result`
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string to process.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - the transform does not exist
    /// - any token execution fails (propagated from `parse_token`)
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError>;

    /// Executes a single token over `input`, without registering it into the processor.
    ///
    /// This is a convenience method for ad-hoc transformations:
    /// - calls `token.transform(input)`
    /// - stores any encountered error in the internal error manager (in your impl)
    ///
    /// # Parameters
    /// - `token`: The token to execute once.
    /// - `input`: Input string.
    ///
    /// # Errors
    /// Returns `Err` if the token's `transform` fails.
    fn process_single(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError>;

    /// Executes a registered transform like `process_all`, but prints each step.
    ///
    /// Intended for debugging and teaching:
    /// - prints an SBS header
    /// - for each token:
    ///   - computes `temp = parse_token(...)`
    ///   - prints: step index, instruction (`to_textforge_line()`), before, after
    /// - returns the final result
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string.
    ///
    /// # Errors
    /// Same error behavior as `process_all`.
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError>;

    /// Executes a single token like `process_single`, but prints a single SBS step.
    ///
    /// Intended for debugging token behavior in isolation.
    ///
    /// Prints:
    /// - Step 0 -> 1
    /// - Instruction (`to_textforge_line()`)
    /// - Before / After
    ///
    /// # Errors
    /// Returns `Err` if the token's `transform` fails.
    fn process_single_with_debug(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError>;

    /// Reads an owned UTF-8 document from `file_path`, runs `pipeline_id`, and
    /// writes the result to `output_path`. No memory mapping or unsafe file-backed
    /// references are used. Output parent directories are created as needed.
    ///
    /// # Parameters
    /// - `pipeline_id`: ID of a transform previously registered via `add_transform` /
    ///   `build()` / `get_pipeline_from_file` / `read_from_bytecode_file`.
    /// - `file_path`: Path to the input file to read. Must already exist.
    /// - `output_path`: Path to write the transformed output to.
    ///
    /// # Errors
    /// Returns `Err` if:
    /// - `file_path` fails to open or read
    /// - the file's contents are not valid UTF-8
    /// - `pipeline_id` does not match any registered transform, or any token fails during
    ///   execution
    /// - `output_path`'s parent directory fails to be created
    /// - `output_path` fails to open or write
    fn process_file(
        &mut self,
        pipeline_id: &str,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(), TextForgeError>;

    /// Runs a pipeline over many `(input file, output file)` pairs in parallel.
    ///
    /// Each task is `(origin, pipeline_id, target)`:
    /// - `origin`: path to the input file to read. Must already exist.
    /// - `pipeline_id`: ID of a transform previously registered via `add_transform` /
    ///   `build()` / `get_pipeline_from_file` / `read_from_bytecode_file`.
    /// - `target`: path to write the transformed output to. Its parent directory is
    ///   created automatically if it doesn't exist yet.
    ///
    /// Tasks are distributed across a `rayon` thread pool (`into_par_iter`), so origin
    /// files are read, transformed, and written concurrently. Tasks are fully
    /// independent: one task failing does not stop or affect the others, and the
    /// returned `Vec` preserves the same order as the input `tasks` vector, so
    /// `results[i]` always corresponds to `tasks[i]`.
    ///
    /// Internally, for each task:
    /// - checks that `origin` exists and is a file (not a directory)
    /// - reads an owned UTF-8 document from `origin`
    /// - runs the registered pipeline via `run_transform(pipeline_id, &input)`
    /// - creates `target`'s parent directory if it doesn't exist yet (`create_dir_all`)
    /// - writes the result to `target` with `std::fs::write`
    ///
    /// Neither `origin` nor `target` are required to have any particular file
    /// extension.
    ///
    /// # Parameters
    /// - `tasks`: the `(origin, pipeline_id, target)` triples to process.
    ///
    /// # Returns
    /// One `Result<(), TextForgeError>` per task, in the same order as `tasks`.
    ///
    /// # Errors
    /// A task's `Err` can come from:
    /// - `origin` not existing (`FileNotFound`) or not being a file (`ValidationError`)
    /// - `origin` failing to read (permissions, invalid UTF-8, ...) (`FileReadingError`)
    /// - `pipeline_id` not matching any registered transform (`TokenArrayNotFound`)
    /// - any token in the pipeline failing during execution
    /// - `target`'s parent directory failing to be created (`FileWritingError`)
    /// - `target` failing to write (`FileWritingError`)
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>>;

    /// Executes a registered transform like [`process_all`](Self::process_all), but also runs
    /// `watcher_list`'s diagnostics after each step and exports the resulting report as JSON.
    ///
    /// For every step after the first, a [`WatcherContext`](crate::watchers::WatcherContext) is built from the *previous* step's
    /// `before`/`current` values plus the *current* step's output (as that previous step's
    /// `after`), then passed to `watcher_list.run_watchers`. The final step has no following
    /// step to supply an `after`, so its context is run with `after = None`. Once every step has
    /// been processed, the report is written to `report_path` via `watcher_list.to_json`, and
    /// `watcher_list` is reset so it can be reused for the next call.
    ///
    /// Available only with the `watchers` feature.
    ///
    /// # Parameters
    /// - `id`: Transform identifier.
    /// - `input`: Input string to process.
    /// - `watcher_list`: The watchers to run after each step.
    /// - `report_path`: Destination path for the exported JSON report.
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist, any token execution fails, a watcher run
    /// fails, or the report fails to serialize/write.
    #[cfg(feature = "watchers")]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError>;

    // -----------------------------------------------------------------
    // Bytecode persistence (feature = "bytecode")
    //
    // These two methods only move a pipeline in and out of the binary `.textforgebc`
    // format — they never process input directly. Once a pipeline is registered (from
    // bytecode, text, or built in-memory), any `process_*` method above can run it.
    // -----------------------------------------------------------------

    /// Writes a registered transform to a `.textforgebc` bytecode file.
    ///
    /// Available only with the `bytecode` feature.
    ///
    /// Internally:
    /// - looks up `id` in `self.transforms`
    /// - calls `write_bytecode_to_file(path, tokens.to_vec())`
    ///
    /// # Errors
    /// Returns `Err` if the transform does not exist or bytecode writing fails.
    #[cfg(feature = "bytecode")]
    fn write_to_bytecode_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError>;

    /// Reads a `.textforgebc` bytecode file and registers it as a new transform, returning
    /// its ID.
    ///
    /// Available only with the `bytecode` feature.
    ///
    /// Internally:
    /// - parses tokens via `read_bytecode_from_file(path)`
    /// - registers them using `add_transform`
    ///
    /// This only registers the pipeline — it does not process any input. To run it, pass the
    /// returned ID to one of the `process_*` methods (e.g. `process_all`,
    /// `process_all_with_debug`).
    ///
    /// # Returns
    /// The UUID string identifying the newly registered transform.
    ///
    /// # Errors
    /// Returns `Err` if bytecode reading/parsing fails.
    #[cfg(feature = "bytecode")]
    fn read_from_bytecode_file(&mut self, path: &Path) -> Result<String, TextForgeError>;
}

/// Creates an empty processor using the standard default-construction trait.
///
/// ```
/// use textforge::api::{processor::TextForgeProcessor, TextForgeBuilderMethods};
/// let mut processor = TextForgeProcessor::default();
/// let id = processor.create_pipeline().trim_both_sides().unwrap().build().unwrap();
/// assert!(!id.is_empty());
/// ```
impl Default for TextForgeProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextForgeProcessor {
    /// Creates a new empty processor.
    ///
    /// No transforms are registered initially.
    pub fn new() -> Self {
        TextForgeProcessor {
            transforms: HashMap::new(),
            profile: ExecutionProfile::balanced(),
        }
    }

    /// This processor using `profile` (see [`ExecutionProfile`]).
    pub fn with_profile(mut self, profile: ExecutionProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Use `profile` for later runs. Results never depend on the profile.
    pub fn set_profile(&mut self, profile: ExecutionProfile) {
        self.profile = profile;
    }

    /// The profile in use (`balanced` by default).
    pub fn profile(&self) -> &ExecutionProfile {
        &self.profile
    }

    /// Set the name of a registered pipeline, read by the DSL as
    /// `__PIPELINE_NAME__`. A pipeline's name defaults to its identifier, or to
    /// the file name without extension when it was loaded from a file.
    ///
    /// ```rust
    /// use textforge::api::TextForgeBuilderMethods;
    /// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
    ///
    /// let mut processor = TextForgeProcessor::new();
    /// let id = processor.create_pipeline().add_to_end("@{{__PIPELINE_NAME__}}")?.build()?;
    /// assert_eq!(processor.pipeline_name(&id)?, id);
    /// processor.set_pipeline_name(&id, "cleanup")?;
    /// assert_eq!(processor.pipeline_name(&id)?, "cleanup");
    /// assert_eq!(processor.process_all(&id, "run")?, "run@cleanup");
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn set_pipeline_name(
        &mut self,
        id: &str,
        name: impl Into<String>,
    ) -> Result<(), TextForgeError> {
        let pipeline = self
            .transforms
            .get_mut(id)
            .ok_or_else(token_array_not_found(id))?;
        pipeline.statics = Arc::new(StaticContext::new(name.into(), None));
        Ok(())
    }

    /// The name of a registered pipeline (see [`Self::set_pipeline_name`]).
    pub fn pipeline_name(&self, id: &str) -> Result<&str, TextForgeError> {
        self.transforms
            .get(id)
            .map(|pipeline| pipeline.statics.pipeline_name())
            .ok_or_else(token_array_not_found(id))
    }

    /// The instructions of pipeline `id` and a fresh execution context for one
    /// run of it. Borrows only the pipeline table, so callers can still use
    /// the error manager.
    fn pipeline_run<'p>(
        transforms: &'p HashMap<String, RegisteredPipeline>,
        id: &str,
        input_file: Option<&Path>,
        limits: crate::api::Limits,
    ) -> Result<(&'p [TokenWrapper], GlobalExecutionContext), TextForgeError> {
        let pipeline = transforms.get(id).ok_or_else(token_array_not_found(id))?;
        let statics = match input_file {
            None => Arc::clone(&pipeline.statics),
            Some(path) => Arc::new(pipeline.statics.with_input_file(path)),
        };
        let mut context = GlobalExecutionContext::with_static(statics);
        context.set_limits(limits);
        context.set_base_dir(pipeline.base_dir.clone());
        Ok((&pipeline.tokens, context))
    }

    /// Make relative paths of the file instructions in pipeline `id` start from
    /// the directory of `file`, the file it was read from.
    fn set_base_dir_from_file(&mut self, id: &str, file: &Path) {
        let directory = file
            .canonicalize()
            .ok()
            .and_then(|file| file.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| file.parent().map(Path::to_path_buf).unwrap_or_default());
        if let Some(pipeline) = self.transforms.get_mut(id) {
            pipeline.base_dir = Some(Arc::from(directory.as_path()));
        }
    }

    /// Name a pipeline loaded from `path` after the file name without extension.
    fn name_after_file(&mut self, id: &str, path: &Path) {
        if let (Some(stem), Some(pipeline)) = (path.file_stem(), self.transforms.get_mut(id)) {
            pipeline.statics = Arc::new(StaticContext::new(
                stem.to_string_lossy().into_owned(),
                None,
            ));
        }
    }

    /// Creates a `TextForgeBuilder` bound to this processor.
    ///
    /// The builder accumulates tokens and, when `build()` is called, it registers a new
    /// transform entry inside this processor and returns the corresponding transform ID.
    ///
    /// # Example
    ///
    /// ```rust
    /// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
    /// use textforge::api::TextForgeBuilderMethods;
    ///
    /// let mut processor = TextForgeProcessor::new();
    ///
    /// let id = processor
    ///     .create_pipeline()
    ///     .trim_both_sides()?
    ///     .add_to_end("!")?
    ///     .build().unwrap();
    ///
    /// let out = processor.process_all(&id, "   banana   ")?;
    /// assert_eq!(out, "banana!");
    /// # Ok::<(), textforge::utils::errors::TextForgeError>(())
    /// ```
    pub fn create_pipeline(&mut self) -> TextForgeBuilder<'_> {
        TextForgeBuilder::new(self)
    }
}

impl TextForgeProcessorMethods for TextForgeProcessor {
    // -- Pipeline persistence (text format) --------------------------

    fn write_pipeline_to_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError> {
        let tokens = match self
            .transforms
            .get(id)
            .map(|pipeline| &pipeline.tokens)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                return Err(e);
            }
        };

        write_to_file(Path::new(path), tokens)
    }

    fn get_pipeline_from_file(&mut self, path: &Path) -> Result<String, TextForgeError> {
        // Joined pipelines (`ppjn`) load now, relative to this file.
        let tokens = read_from_file(Path::new(path)).and_then(|mut tokens| {
            let file = path.canonicalize().map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileNotFound(
                        format!("cannot open the pipeline file `{}`: {e}", path.display()).into(),
                    ),
                    "processor",
                    path.display().to_string(),
                )
            })?;
            let directory = file.parent().unwrap_or(Path::new("."));
            crate::parser::joins::link(&mut tokens, directory, std::slice::from_ref(&file))?;
            Ok(tokens)
        });
        let tokens = match tokens {
            Ok(x) => x,
            Err(e) => {
                return Err(e);
            }
        };

        let identifier = self.add_transform(tokens);
        self.name_after_file(&identifier, path);
        self.set_base_dir_from_file(&identifier, path);
        Ok(identifier)
    }

    // -- Pipeline registration & inspection ---------------------------

    fn add_transform(&mut self, mut tokens: Vec<TokenWrapper>) -> String {
        // Load joined pipelines (`ppjn`) still unloaded, from the working
        // directory. This method cannot report errors: a join that fails to
        // load here is loaded, and reports its error, when it runs.
        for token in &mut tokens {
            let _ = crate::parser::joins::link(std::slice::from_mut(token), Path::new("."), &[]);
        }
        for token in &mut tokens {
            token.prepare_constant();
        }
        let identifier = Uuid::new_v4().to_string();
        let statics = Arc::new(StaticContext::new(identifier.clone(), None));
        self.transforms.insert(
            identifier.clone(),
            RegisteredPipeline {
                tokens,
                statics,
                base_dir: None,
            },
        );
        identifier
    }

    fn remove_transform(&mut self, id: &str) -> Result<(), TextForgeError> {
        match self.transforms.remove(id).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::TokenArrayNotFound(
                    format!(
                        "cannot remove the pipeline `{id}`: no pipeline is registered with that id"
                    )
                    .into(),
                ),
                "processor",
                id.to_string(),
            )
        }) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    fn show_transforms(&self) {
        for (i, k) in self.transforms.keys().enumerate() {
            println!("{} - {}", i, k);
        }
    }

    fn transform_exists(&self, id: &str) -> bool {
        self.transforms.contains_key(id)
    }

    fn get_transform_vec(&self, id: &str) -> Result<Vec<TokenWrapper>, TextForgeError> {
        Ok(self
            .transforms
            .get(id)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::TokenArrayNotFound(
                        format!("no pipeline is registered with the id `{id}`").into(),
                    ),
                    "processor",
                    id.to_string(),
                )
            })?
            .tokens
            .iter()
            .map(TokenWrapper::without_preparation)
            .collect())
    }

    fn get_text_transform_vec(&self, id: &str) -> Result<Vec<String>, TextForgeError> {
        self.transforms
            .get(id)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::TokenArrayNotFound(
                        format!("no pipeline is registered with the id `{id}`").into(),
                    ),
                    "processor",
                    id.to_string(),
                )
            })?
            .tokens
            .iter()
            .map(|t| t.to_text_line_unresolved().map(|s| s.to_string()))
            .collect::<Result<Vec<String>, TextForgeError>>()
    }

    // -- Processing ----------------------------------------------------

    fn run_transform<'a>(
        &self,
        id: &str,
        input: Cow<'a, str>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        self.run_transform_from(id, input, None)
    }

    #[cfg(feature = "buffered")]
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        self.process_all_via_buffer(id, input, None)
    }

    #[cfg(not(feature = "buffered"))]
    fn process_all(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        match self.run_transform(id, Cow::from(input)) {
            Ok(result) => Ok(result.into_owned()),
            Err(e) => Err(e),
        }
    }

    fn process_single(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let mut context = GlobalExecutionContext::new();
        context.set_limits(self.profile.limits);
        match token.apply_token(Cow::from(input), &mut context) {
            Ok(x) => Ok(x.to_string()),
            Err(e) => Err(e),
        }
    }

    #[cfg(feature = "buffered")]
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        self.process_debug_from(id, input, None, &mut std::io::stdout().lock())
    }

    #[cfg(not(feature = "buffered"))]
    fn process_all_with_debug(&mut self, id: &str, input: &str) -> Result<String, TextForgeError> {
        self.process_debug_from(id, input, None, &mut std::io::stdout().lock())
    }

    fn process_single_with_debug(
        &mut self,
        token: TokenWrapper,
        input: &str,
    ) -> Result<String, TextForgeError> {
        let mut ctx = GlobalExecutionContext::new();
        ctx.set_limits(self.profile.limits);
        ctx.enable_trace();
        let output = match token.apply_token(Cow::from(input), &mut ctx) {
            Ok(x) => x,
            Err(e) => {
                return Err(e);
            }
        };
        println!(
            "Step: [{}] => [{}]\nInstruction: {}\nBefore: {}\nAfter: {}\n",
            (0).to_string().blue(),
            (1).to_string().blue(),
            token.to_textforge_line().yellow(),
            input.red(),
            output.green()
        );
        for nested in ctx.take_trace_lines() {
            println!("{nested}");
        }

        Ok(output.to_string())
    }

    fn process_file(
        &mut self,
        pipeline_id: &str,
        file_path: &Path,
        output_path: &Path,
    ) -> Result<(), TextForgeError> {
        use std::fs::File;

        let output_path_str = output_path.to_string_lossy().into_owned();
        let input = file_io::read_utf8_file(file_path, "processor")?;

        // Transfer ownership to the whole-document engine instead of cloning input.
        #[cfg(not(feature = "buffered"))]
        let result = self
            .run_transform_from(pipeline_id, Cow::Owned(input), Some(file_path))
            .map(Cow::into_owned)?;
        #[cfg(feature = "buffered")]
        let result = self.process_all_via_buffer(pipeline_id, &input, Some(file_path))?;

        // Create the output directory first, as process_batch does for each target.
        if let Some(parent) = output_path.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            std::fs::create_dir_all(parent).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileWritingError(
                        format!(
                            "cannot create the output directory `{}`: {e}",
                            parent.display()
                        )
                        .into(),
                    ),
                    Cow::from("processor"),
                    output_path_str.clone(),
                )
            })?;
        }

        let mut out = File::create(output_path).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileOpeningError(
                    format!("cannot create the output file `{output_path_str}`: {e}").into(),
                ),
                Cow::from("processor"),
                output_path_str.clone(),
            )
        })?;

        out.write_all(result.as_bytes()).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileWritingError(
                    format!("cannot write the output file `{output_path_str}`: {e}").into(),
                ),
                Cow::from("processor"),
                output_path_str.clone(),
            )
        })?;

        Ok(())
    }

    #[cfg(feature = "buffered")]
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>> {
        self.process_batch_via_buffer(tasks)
    }

    #[cfg(not(feature = "buffered"))]
    fn process_batch(&self, tasks: Vec<(&Path, &str, &Path)>) -> Vec<Result<(), TextForgeError>> {
        tasks
            .into_par_iter()
            .map(
                |(origin, pipeline_id, target)| -> Result<(), TextForgeError> {
                    if !origin.exists() {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::FileNotFound(
                                format!("the input file `{}` does not exist", origin.display())
                                    .into(),
                            ),
                            Cow::Borrowed("processor"),
                            origin.display().to_string(),
                        ));
                    }

                    if !origin.is_file() {
                        return Err(TextForgeError::new(
                            TextForgeErrorCode::ValidationError(
                                format!("the input path `{}` is not a file", origin.display())
                                    .into(),
                            ),
                            Cow::Borrowed("processor"),
                            origin.display().to_string(),
                        ));
                    }

                    let input = file_io::read_utf8_file(origin, "processor")?;
                    let output =
                        self.run_transform_from(pipeline_id, Cow::Owned(input), Some(origin))?;

                    if let Some(parent) = target.parent()
                        && !parent.as_os_str().is_empty()
                        && !parent.exists()
                    {
                        std::fs::create_dir_all(parent).map_err(|e| {
                            TextForgeError::new(
                                TextForgeErrorCode::FileWritingError(
                                    format!(
                                        "cannot create the output directory `{}`: {e}",
                                        parent.display()
                                    )
                                    .into(),
                                ),
                                Cow::Borrowed("processor"),
                                target.display().to_string(),
                            )
                        })?;
                    }

                    std::fs::write(target, output.as_bytes()).map_err(|e| {
                        TextForgeError::new(
                            TextForgeErrorCode::FileWritingError(
                                format!("cannot write the output file `{}`: {e}", target.display())
                                    .into(),
                            ),
                            Cow::Borrowed("processor"),
                            target.display().to_string(),
                        )
                    })?;

                    Ok(())
                },
            )
            .collect()
    }
    #[cfg(all(feature = "watchers", feature = "buffered"))]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        self.process_watchers_via_buffer(id, input, None, watcher_list, report_path)
    }

    #[cfg(all(feature = "watchers", not(feature = "buffered")))]
    fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        self.process_watchers_from(id, input, None, watcher_list, report_path)
    }

    // -- Bytecode persistence (feature = "bytecode") --------------------
    //
    // Only read/write `.textforgebc`; never process input directly.

    #[cfg(feature = "bytecode")]
    fn write_to_bytecode_file(&mut self, id: &str, path: &Path) -> Result<(), TextForgeError> {
        let tokens = match self
            .transforms
            .get(id)
            .map(|pipeline| &pipeline.tokens)
            .ok_or_else(token_array_not_found(id))
        {
            Ok(x) => x,
            Err(e) => {
                return Err(e);
            }
        };

        write_bytecode_to_file(path, tokens.to_vec())
    }

    #[cfg(feature = "bytecode")]
    fn read_from_bytecode_file(&mut self, path: &Path) -> Result<String, TextForgeError> {
        let tokens = match read_bytecode_from_file(path) {
            Ok(x) => x,
            Err(e) => {
                return Err(e);
            }
        };

        let identifier = self.add_transform(tokens.to_vec());
        self.name_after_file(&identifier, path);
        self.set_base_dir_from_file(&identifier, path);

        Ok(identifier)
    }
}

#[cfg(all(feature = "watchers", feature = "buffered", feature = "cli"))]
impl TextForgeProcessor {
    /// [`TextForgeProcessorMethods::process_all_with_watchers`] for a document
    /// read from `input_file` (exposed to the DSL as `__INPUT_FILE__`).
    pub(crate) fn process_watchers_from(
        &mut self,
        id: &str,
        input: &str,
        input_file: Option<&Path>,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        self.process_watchers_via_buffer(id, input, input_file, watcher_list, report_path)
    }
}

#[cfg(feature = "cli")]
impl TextForgeProcessor {
    /// Make every run of pipeline `id` see `input_file` (`__INPUT_FILE__`) until
    /// [`restore_statics`](Self::restore_statics) puts back the returned
    /// context. `txf` runs a pipeline once per line or chunk; building the
    /// context once instead of in every run (as passing the file to each run
    /// does) keeps line mode as fast as without an input file.
    pub(crate) fn statics_for_input(
        &mut self,
        id: &str,
        input_file: &Path,
    ) -> Option<Arc<StaticContext>> {
        let pipeline = self.transforms.get_mut(id)?;
        let with_file = Arc::new(pipeline.statics.with_input_file(input_file));
        Some(std::mem::replace(&mut pipeline.statics, with_file))
    }

    /// Put back the context [`statics_for_input`](Self::statics_for_input)
    /// returned.
    pub(crate) fn restore_statics(&mut self, id: &str, previous: Arc<StaticContext>) {
        if let Some(pipeline) = self.transforms.get_mut(id) {
            pipeline.statics = previous;
        }
    }
}

impl TextForgeProcessor {
    /// [`TextForgeProcessorMethods::process_all`] for a document read from
    /// `input_file` (exposed to the DSL as `__INPUT_FILE__`). Takes the input
    /// by value so the whole-document engine can work on it without a copy.
    #[cfg(feature = "cli")]
    pub(crate) fn process_text_from(
        &mut self,
        id: &str,
        input: Cow<'_, str>,
        input_file: Option<&Path>,
    ) -> Result<String, TextForgeError> {
        #[cfg(feature = "buffered")]
        {
            self.process_all_via_buffer(id, &input, input_file)
        }
        #[cfg(not(feature = "buffered"))]
        {
            self.run_transform_from(id, input, input_file)
                .map(Cow::into_owned)
        }
    }

    /// [`TextForgeProcessorMethods::process_all_with_debug`] for a document read
    /// from `input_file`, writing the trace to `log` instead of standard output.
    /// Nothing is written when the pipeline fails.
    pub(crate) fn process_debug_from(
        &mut self,
        id: &str,
        input: &str,
        input_file: Option<&Path>,
        log: &mut (dyn Write + '_),
    ) -> Result<String, TextForgeError> {
        let log_error = |error: std::io::Error| {
            TextForgeError::new(
                TextForgeErrorCode::FileWritingError(
                    format!("cannot write the debug log: {error}").into(),
                ),
                "processor",
                "",
            )
        };
        #[cfg(feature = "buffered")]
        let result = self.process_debug_via_buffer(id, input, input_file, log)?;
        #[cfg(not(feature = "buffered"))]
        let result = {
            let (result, text) = self.debug_log(id, input, input_file)?;
            // A single write.
            log.write_all(text.as_bytes()).map_err(log_error)?;
            result
        };
        // Without steps the trace would end at its heading, with no `After:`
        // holding the final text.
        if self
            .transforms
            .get(id)
            .is_some_and(|pipeline| pipeline.tokens.is_empty())
        {
            log.write_all(b"The pipeline has no instructions: the text is unchanged.\n")
                .map_err(log_error)?;
        }
        Ok(result)
    }

    /// [`TextForgeProcessorMethods::run_transform`] for a document read from
    /// `input_file` (exposed to the DSL as `__INPUT_FILE__`).
    fn run_transform_from<'a>(
        &self,
        id: &str,
        input: Cow<'a, str>,
        input_file: Option<&Path>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let (tokens, mut context) =
            Self::pipeline_run(&self.transforms, id, input_file, self.profile.limits)?;

        run_tokens(
            tokens,
            input,
            &mut context,
            |_, token, document, context| token.apply_token(document, context),
        )
    }
}

#[cfg(all(feature = "watchers", not(feature = "buffered")))]
impl TextForgeProcessor {
    /// [`TextForgeProcessorMethods::process_all_with_watchers`] for a document
    /// read from `input_file` (exposed to the DSL as `__INPUT_FILE__`).
    pub(crate) fn process_watchers_from(
        &mut self,
        id: &str,
        input: &str,
        input_file: Option<&Path>,
        watcher_list: &mut WatcherList,
        report_path: &Path,
    ) -> Result<String, TextForgeError> {
        let (tokens, mut context) =
            match Self::pipeline_run(&self.transforms, id, input_file, self.profile.limits) {
                Ok(x) => x,
                Err(e) => {
                    return Err(e);
                }
            };
        // The pipeline state is an Arc<str>, not a String: reusing `current_arc` from
        // one step as the `before` of the next is an Arc::clone (a refcount bump),
        // not a text copy. apply_token still takes a &str
        // (Arc<str> derefs to str, so &current_arc works).
        let current_arc: Arc<str> = Arc::from(input);

        // Holds (before, current, instruction) of the previous step until its
        // "after" is known, which happens only after the next step runs.
        let mut pending: Option<(Arc<str>, Arc<str>, String)> = None;

        let current_arc = run_tokens(
            tokens,
            current_arc,
            &mut context,
            |_, token, current_arc, context| {
                let before_arc = Arc::clone(&current_arc); // O(1)

                let after = token.apply_token(Cow::Borrowed(current_arc.as_ref()), context)?;

                // The only whole-text copy per step: the transform's output
                // (a new String, allocated by the instruction itself, which is
                // inherent to the instruction's work) must also exist as an
                // Arc<str> to become this step's "current" and the next step's "before".
                let current_arc: Arc<str> = Arc::from(after.as_ref());

                if let Some((prev_before, prev_current, prev_instruction)) = pending.take() {
                    let watcher_ctx = WatcherContext::new(
                        prev_current,
                        prev_before,
                        Some(Arc::clone(&current_arc)), // O(1)
                        prev_instruction,
                    );
                    watcher_list.run_watchers(watcher_ctx)?;
                }

                pending = Some((
                    before_arc,
                    Arc::clone(&current_arc), // O(1)
                    token.to_textforge_line().to_string(),
                ));
                Ok(current_arc)
            },
        )?;

        // Last instruction: there is no next step, so after = None.
        if let Some((before, current, instruction)) = pending.take() {
            let watcher_ctx = WatcherContext::new(current, before, None::<Arc<str>>, instruction);
            watcher_list.run_watchers(watcher_ctx)?;
        }

        watcher_list.to_json(report_path)?;
        watcher_list.reset();

        Ok(current_arc.to_string())
    }
}

#[cfg(not(feature = "buffered"))]
impl TextForgeProcessor {
    /// Run `id` with debug output and return `(result, log)`; nested steps of
    /// `cblk`, `iter`, `on*` and `rmape` follow each top-level step.
    fn debug_log(
        &mut self,
        id: &str,
        input: &str,
        input_file: Option<&Path>,
    ) -> Result<(String, String), TextForgeError> {
        let dashes = 10;

        let (tokens, mut context) =
            match Self::pipeline_run(&self.transforms, id, input_file, self.profile.limits) {
                Ok(x) => x,
                Err(e) => {
                    return Err(e);
                }
            };

        let mut log = String::new();
        log.push_str("PROCESSING STEP BY STEP:\n");
        log.push_str(&"-".repeat(dashes));
        log.push_str("\n\n");
        context.enable_trace();

        let result = run_tokens(
            tokens,
            Cow::from(input),
            &mut context,
            |index, token, result, context| {
                let counter = index as i64;
                let bound = token
                    .bind_for_debug(context, crate::context::builtins::CurrentText::Str(&result))?;
                let line = bound.to_textforge_line();
                let red_result = result.red();
                let temp = bound
                    .transform(result, Some(context))
                    .map_err(|error| token.locate_bound(error, bound.as_ref()))?;

                if token.get_string_repr() == "blk" {
                    let v = match bound.get_params().first() {
                        Some(crate::parser::params::TextForgeParamTypes::String(name)) => name,
                        _ => {
                            return Err(TextForgeError::new(
                                TextForgeErrorCode::InvalidParameters(
                                    "internal error: a `blk` instruction has no block name".into(),
                                ),
                                "blk",
                                "",
                            ));
                        }
                    };

                    log.push_str(
                        &format!(
                            "Step: [{}] => [{}]\n{}\n\tBlock Instruction: {}\t\tBlock Name: {}\n\t\t\tCurrent instructions Associated to this Block:\n{}",
                            counter.to_string().blue(),
                            (counter + 1).to_string().blue(),
                            "Block Declaration: ".to_string().green(),
                            line.yellow(),
                            v.to_string().green(),
                            context.get_formatted_block_items(v)?
                        )
                    );
                } else {
                    log.push_str(&format!(
                        "Step: [{}] => [{}]\nInstruction: {}\nBefore: {}\nAfter: {}\n\n",
                        counter.to_string().blue(),
                        (counter + 1).to_string().blue(),
                        line.yellow(),
                        red_result,
                        temp.green()
                    ));
                }

                let nested = context.take_trace_lines();
                for line in &nested {
                    log.push_str(line);
                    log.push('\n');
                }
                if !nested.is_empty() {
                    log.push('\n');
                }

                if index + 1 < tokens.len() {
                    log.push_str(&"-".repeat(dashes));
                    log.push_str("\n\n");
                }

                Ok(temp)
            },
        )?;

        Ok((result.into_owned(), log))
    }
}

#[cfg(all(test, not(feature = "buffered")))]
mod debug_tests {
    use super::*;
    use crate::context::debug_trace::fixtures::{NESTED_DEBUG_PIPELINE, NESTED_DEBUG_TRACE};

    #[test]
    fn unbuffered_debug_traces_nested_steps_like_buffered_debug() {
        let mut processor = TextForgeProcessor::new();
        let tokens = NESTED_DEBUG_PIPELINE
            .lines()
            .map(|line| crate::text::reader::read_from_text(line).unwrap())
            .collect();
        let id = processor.add_transform(tokens);
        let input = "  cat   wolf  \nx";
        let (output, log) = processor.debug_log(&id, input, None).unwrap();
        assert_eq!(output, processor.process_all(&id, input).unwrap());
        let ansi = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
        let log = ansi.replace_all(&log, "");
        let mut rest: &str = &log;
        for expected in NESTED_DEBUG_TRACE {
            let at = rest
                .find(expected)
                .unwrap_or_else(|| panic!("missing {expected:?} in {log}"));
            rest = &rest[at + expected.len()..];
        }
    }
}
