//! Running the pipeline of a round with the public TextForge API.

use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use textforge::{
    api::{
        ExecutionProfile, Limits,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// Largest text a round may produce (`rpt`, `padl`, `padr`).
pub const MAX_GENERATED_BYTES: u64 = 1 << 20;
/// Largest `iter` count in a round.
pub const MAX_ITERATIONS: u64 = 100_000;

static NEXT_FILE: AtomicU64 = AtomicU64::new(0);

/// Runs pipelines written as text lines on the text of a round.
///
/// The library reads text pipelines from files only, so the engine keeps one
/// temporary `.textforge` file, rewritten before each run and removed on drop.
/// The whole pipeline runs from the start text every time, so variables and
/// blocks defined by earlier instructions keep working. Limits are much lower
/// than the library defaults: a round is a short text.
pub struct Engine {
    path: PathBuf,
    profile: ExecutionProfile,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

impl Engine {
    /// An engine with its own temporary file.
    pub fn new() -> Self {
        let name = format!(
            "txfg-{}-{}.textforge",
            std::process::id(),
            NEXT_FILE.fetch_add(1, Ordering::Relaxed)
        );
        Engine {
            path: std::env::temp_dir().join(name),
            profile: ExecutionProfile {
                limits: Limits {
                    max_generated_bytes: Some(MAX_GENERATED_BYTES),
                    max_iterations: Some(MAX_ITERATIONS),
                },
                ..ExecutionProfile::balanced()
            },
        }
    }

    /// Runs `pipeline` (one instruction per item, each ending with `;`) on
    /// `input` and returns the resulting text.
    pub fn run(&self, pipeline: &[String], input: &str) -> Result<String, TextForgeError> {
        if pipeline.is_empty() {
            return Ok(input.to_owned());
        }
        let mut source = pipeline.join("\n");
        source.push('\n');
        fs::write(&self.path, source).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileWritingError(
                    format!(
                        "cannot write the round pipeline to `{}`: {e}",
                        self.path.display()
                    )
                    .into(),
                ),
                "txfg",
                "",
            )
        })?;
        let mut processor = TextForgeProcessor::new().with_profile(self.profile);
        let id = processor.get_pipeline_from_file(&self.path)?;
        processor.process_all(&id, input)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn runs_the_whole_pipeline_from_the_start() {
        let engine = Engine::new();
        let result = engine.run(&lines(&["tbs;", "ate \" fruit!\";", "tua;"]), " banana    ");
        assert_eq!(result.unwrap(), "BANANA FRUIT!");
        assert_eq!(engine.run(&[], "é🙂").unwrap(), "é🙂");
        assert_eq!(engine.run(&lines(&["rev;"]), "aé🙂").unwrap(), "🙂éa");
    }

    #[test]
    fn reports_errors_instead_of_panicking() {
        let engine = Engine::new();
        assert!(engine.run(&lines(&["nope;"]), "x").is_err());
        assert!(engine.run(&lines(&["dls 9;"]), "x").is_err());
        assert!(engine.run(&lines(&["rpt 100000000;"]), "abc").is_err());
    }

    #[test]
    fn removes_its_file() {
        let engine = Engine::new();
        let path = engine.path.clone();
        engine.run(&lines(&["tua;"]), "a").unwrap();
        assert!(path.exists());
        drop(engine);
        assert!(!path.exists());
    }
}
