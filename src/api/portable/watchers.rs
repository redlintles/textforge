use super::processor::{conversion, execution, persistence};
use super::{BindingError, ErrorStage, FilePath, PortableProcessor};
use crate::{
    api::processor::TextForgeProcessorMethods,
    utils::errors::{TextForgeError, TextForgeErrorCode},
    watchers::{ExecutionWindows, WatcherContext, WatcherList},
};

/// Owned watcher observations with the engine's previous/current/next semantics.
/// `after` is absent on the last observed instruction. See [`Observer`].
///
/// ```
/// # #[cfg(feature = "watchers")] {
/// use textforge::api::portable::Observation;
/// let observation = Observation { current: "é".into(), before: "".into(), after: None, instruction: "atb".into() };
/// assert_eq!(observation.current.chars().count(), 1);
/// # }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// The text after the step.
    pub current: String,
    /// The text before the step.
    pub before: String,
    /// The text after the next step, or `None` for the last one.
    pub after: Option<String>,
    /// The step's instruction, as text.
    pub instruction: String,
}

impl From<WatcherContext> for Observation {
    fn from(context: WatcherContext) -> Self {
        Self {
            current: context.current.to_string(),
            before: context.before.to_string(),
            after: context.after.map(|text| text.to_string()),
            instruction: context.instruction.to_string(),
        }
    }
}

/// Runtime callback bridge. Adapters only translate the invocation and return a
/// value/error; scheduling, ownership and report generation remain in TextForge.
/// Callbacks run synchronously on the thread invoking watched processing.
///
/// ```
/// # #[cfg(feature = "watchers")] {
/// use textforge::api::portable::{Observation, Observers};
/// let mut observers = Observers::new();
/// observers.set("bytes".into(), Box::new(|context: Observation| Ok(context.current.len().to_string())));
/// observers.schedule_all("bytes").unwrap();
/// # }
/// ```
pub trait Observer {
    /// Returns the value to record for this step, or an error message.
    fn observe(&self, context: Observation) -> Result<String, String>;
}

impl<F: Fn(Observation) -> Result<String, String>> Observer for F {
    fn observe(&self, context: Observation) -> Result<String, String> {
        self(context)
    }
}

/// Owned watcher registration, dynamic names, scheduling, reset and JSON export.
/// This object is thread-confined, matching the core watcher list. Replacement
/// preserves scheduling; successful processor runs reset results, not callbacks.
///
/// ```
/// # #[cfg(feature = "watchers")] {
/// use textforge::api::portable::Observers;
/// let mut observers = Observers::new();
/// observers.set_builtin("characters".into(), "current_len_chars").unwrap();
/// observers.schedule_all("characters").unwrap();
/// observers.reset();
/// # }
/// ```
#[derive(Default)]
pub struct Observers {
    core: WatcherList,
}

impl Observers {
    /// No observers.
    pub fn new() -> Self {
        Self::default()
    }
    /// Registers (or replaces) the observer called `name`.
    pub fn set(&mut self, name: String, observer: Box<dyn Observer>) {
        let context_name = name.clone();
        self.core.set_owned_watcher(name, move |context| {
            observer.observe(context.into()).map_err(|message| {
                TextForgeError::new(
                    TextForgeErrorCode::GenericError(message.into()),
                    "portable observer",
                    context_name.clone(),
                )
            })
        });
    }
    /// Registers a built-in metric (see [`builtin_names`](Self::builtin_names)) as `name`.
    pub fn set_builtin(&mut self, name: String, metric: &str) -> Result<(), BindingError> {
        let callback = BUILTINS
            .iter()
            .find(|(key, _)| *key == metric)
            .map(|(_, callback)| *callback)
            .ok_or_else(|| {
                conversion(TextForgeError::new(
                    TextForgeErrorCode::WatcherNotFoundError(
                        format!(
                            "unknown built-in metric `{metric}`; available metrics: {}",
                            BUILTINS
                                .iter()
                                .map(|(key, _)| *key)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                        .into(),
                    ),
                    "",
                    metric.to_string(),
                ))
            })?;
        self.core
            .set_owned_watcher(name, move |context| Ok(callback(context)));
        Ok(())
    }
    /// The names of the built-in metrics.
    pub fn builtin_names() -> Vec<String> {
        BUILTINS
            .iter()
            .map(|(name, _)| (*name).to_string())
            .collect()
    }
    /// Runs observer `name` after every step.
    pub fn schedule_all(&mut self, name: &str) -> Result<(), BindingError> {
        self.core
            .schedule_owned_watcher(name, ExecutionWindows::All)
            .map_err(execution)
    }
    /// Clears the report and keeps the observers.
    pub fn reset(&mut self) {
        self.core.reset();
    }
    /// Runs the scheduled observers on one observation.
    pub fn run(&mut self, observation: Observation) -> Result<(), BindingError> {
        self.core
            .run_watchers(WatcherContext::new(
                observation.current,
                observation.before,
                observation.after,
                observation.instruction,
            ))
            .map_err(watcher_error)
    }
    /// Writes the report as JSON.
    pub fn write_report(&self, path: &FilePath) -> Result<(), BindingError> {
        self.core.to_json(path.as_path()).map_err(persistence)
    }
}

impl PortableProcessor {
    /// Runs a pipeline with `observers` and writes their report.
    pub fn process_all_with_watchers(
        &mut self,
        id: &str,
        input: &str,
        observers: &mut Observers,
        report: &FilePath,
    ) -> Result<String, BindingError> {
        self.core
            .process_all_with_watchers(id, input, &mut observers.core, report.as_path())
            .map_err(watcher_error)
    }
}

fn watcher_error(error: TextForgeError) -> BindingError {
    let stage = if error.instruction == "portable observer" {
        ErrorStage::Callback
    } else {
        ErrorStage::Execution
    };
    BindingError::from_core(stage, error)
}

/// Registry entry for all core default metrics, without foreign callback glue.
///
/// ```
/// # #[cfg(feature = "watchers")] {
/// assert!(textforge::api::portable::Observers::builtin_names().contains(&"word_count".to_string()));
/// # }
/// ```
type Builtin = (&'static str, fn(WatcherContext) -> String);
pub(crate) const BUILTINS: &[Builtin] = &[
    (
        "current_len_bytes",
        crate::watchers::default_watchers::current_len_bytes,
    ),
    (
        "current_len_chars",
        crate::watchers::default_watchers::current_len_chars,
    ),
    ("byte_delta", crate::watchers::default_watchers::byte_delta),
    ("char_delta", crate::watchers::default_watchers::char_delta),
    ("word_count", crate::watchers::default_watchers::word_count),
    (
        "word_count_delta",
        crate::watchers::default_watchers::word_count_delta,
    ),
    ("line_count", crate::watchers::default_watchers::line_count),
    ("is_empty", crate::watchers::default_watchers::is_empty),
    (
        "is_unchanged",
        crate::watchers::default_watchers::is_unchanged,
    ),
    (
        "whitespace_count",
        crate::watchers::default_watchers::whitespace_count,
    ),
    (
        "digit_count",
        crate::watchers::default_watchers::digit_count,
    ),
    (
        "alpha_count",
        crate::watchers::default_watchers::alpha_count,
    ),
    (
        "uppercase_count",
        crate::watchers::default_watchers::uppercase_count,
    ),
    (
        "lowercase_count",
        crate::watchers::default_watchers::lowercase_count,
    ),
    (
        "unique_char_count",
        crate::watchers::default_watchers::unique_char_count,
    ),
];
