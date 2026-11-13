//! Safe whole-document reads, atomic writes and caller-owned batch scheduling.
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::Read,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use super::{TextForgeProcessor, TextForgeProcessorMethods};
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// Read a document once, then transfer its allocation into a validated UTF-8 string.
/// File's `read_to_end` uses size information when available; wrapping it in a
/// BufReader would add another buffer without helping this whole-file read.
pub(crate) fn read_utf8_file(
    path: &Path,
    location: &'static str,
) -> Result<String, TextForgeError> {
    let mut file = File::open(path).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileOpeningError(error.to_string().into()),
            location,
            format!("{}: {error}", path.display()),
        )
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileReadingError(error.to_string().into()),
            location,
            format!("{}: {error}", path.display()),
        )
    })?;
    String::from_utf8(bytes).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidUtf8Error(error.utf8_error().to_string().into()),
            location,
            path.to_string_lossy().into_owned(),
        )
    })
}

/// Replace `path` with the bytes `write` produces, atomically.
///
/// The bytes go to a new temporary file in the same directory (so the final
/// rename stays on one file system), which is flushed to disk and then renamed
/// over `path`. If anything fails, the previous file is left untouched and the
/// temporary file is removed. Missing directories are not created. Concurrent
/// writers of the same path never interleave: the last rename wins. Replacing
/// a file gives it default permissions, and a symbolic link at `path` is
/// replaced rather than followed.
pub(crate) fn write_atomically(
    path: &Path,
    location: &'static str,
    write: impl FnOnce(&mut File) -> Result<(), TextForgeError>,
) -> Result<(), TextForgeError> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let error = |code: TextForgeErrorCode, detail: &dyn std::fmt::Display| {
        TextForgeError::new(code, location, format!("{}: {detail}", path.display()))
    };
    let name = path.file_name().ok_or_else(|| {
        error(
            TextForgeErrorCode::FileWritingError("Output path has no file name".into()),
            &"no file name",
        )
    })?;
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !directory.is_dir() {
        return Err(error(
            TextForgeErrorCode::FileNotFound("Output directory does not exist".into()),
            &directory.display(),
        ));
    }
    let mut temporary = OsString::from(".");
    temporary.push(name);
    temporary.push(format!(
        ".{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let temporary = directory.join(temporary);

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| {
                error(
                    TextForgeErrorCode::FileOpeningError(e.to_string().into()),
                    &e,
                )
            })?;
        write(&mut file)?;
        file.sync_all().map_err(|e| {
            error(
                TextForgeErrorCode::FileWritingError(e.to_string().into()),
                &e,
            )
        })?;
        drop(file);
        std::fs::rename(&temporary, path).map_err(|e| {
            error(
                TextForgeErrorCode::FileWritingError(e.to_string().into()),
                &e,
            )
        })
    })();
    if result.is_err() {
        // Best effort: the temporary file may not exist yet.
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

impl TextForgeProcessor {
    /// Execute a batch inside a reusable, caller-owned Rayon thread pool.
    ///
    /// Results retain task order, including individual errors. This also applies
    /// with `buffered` enabled. Reuse the pool across calls to avoid thread creation
    /// per batch. The worker count limits active tasks, not bytes per document;
    /// transforms can still materialize or expand entire documents.
    ///
    /// ```
    /// use textforge::api::processor::TextForgeProcessor;
    /// let processor = TextForgeProcessor::new();
    /// let pool = rayon::ThreadPoolBuilder::new().num_threads(2).build()?;
    /// let results = processor.process_batch_in_pool(&pool, Vec::new());
    /// assert!(results.is_empty());
    /// // Keep `pool` for subsequent batches; the global Rayon pool is unchanged.
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn process_batch_in_pool(
        &self,
        pool: &rayon::ThreadPool,
        tasks: Vec<(&Path, &str, &Path)>,
    ) -> Vec<Result<(), TextForgeError>> {
        pool.install(|| self.process_batch(tasks))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::execution_context::GlobalExecutionContext,
        parser::{params::TextForgeParamTypes, resolve_var::TokenWrapper},
        tokens::{InstructionMethods, instructions::null::Null},
    };
    use std::borrow::Cow;

    /// Test instruction observing the pool used by the actual batch worker.
    #[derive(Clone, Default)]
    struct PoolWidth(Null);

    impl InstructionMethods for PoolWidth {
        fn to_textforge_line(&self) -> Cow<'static, str> {
            self.0.to_textforge_line()
        }
        fn get_string_repr(&self) -> &'static str {
            "null"
        }
        fn get_params(&self) -> &Vec<TextForgeParamTypes> {
            self.0.get_params()
        }
        fn set_params(&mut self, _params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
            Ok(())
        }
        fn transform<'a>(
            &self,
            _input: Cow<'a, str>,
            _context: Option<&mut GlobalExecutionContext>,
        ) -> Result<Cow<'a, str>, TextForgeError> {
            Ok(Cow::Owned(rayon::current_num_threads().to_string()))
        }
        #[cfg(feature = "bytecode")]
        fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
            self.0.to_bytecode()
        }
        #[cfg(feature = "bytecode")]
        fn get_opcode(&self) -> u32 {
            self.0.get_opcode()
        }
    }

    #[test]
    fn actual_instructions_execute_inside_the_supplied_pool() {
        let directory =
            std::env::temp_dir().join(format!("textforge-pool-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&directory).unwrap();
        let input = directory.join("input");
        let output = directory.join("output");
        std::fs::write(&input, "document").unwrap();
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(vec![TokenWrapper::new(
            Box::new(PoolWidth::default()),
            None,
        )]);
        for threads in [1, 3] {
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(threads)
                .build()
                .unwrap();
            for _ in 0..2 {
                let results = processor.process_batch_in_pool(&pool, vec![(&input, &id, &output)]);
                assert!(results[0].is_ok(), "{:?}", results[0]);
                assert_eq!(
                    std::fs::read_to_string(&output).unwrap(),
                    threads.to_string()
                );
            }
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}
