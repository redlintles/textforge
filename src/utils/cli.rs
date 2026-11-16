//! Helpers of the `txf` program: reading modes, runs with an input file and a
//! debug destination, and profile resolution (`.txfrc`, flags). Only with the
//! `cli` feature.
use std::{borrow::Cow, io::Write, path::Path, str::FromStr};

use crate::{
    api::processor::TextForgeProcessor,
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

/// How `txf` feeds the input to a pipeline (`--read-mode`).
///
/// ```
/// use textforge::utils::cli::ReadMode;
///
/// assert_eq!("all".parse::<ReadMode>(), Ok(ReadMode::All));
/// assert_eq!("line".parse::<ReadMode>(), Ok(ReadMode::Line));
/// assert_eq!("chunk-4096".parse::<ReadMode>(), Ok(ReadMode::Chunk(4096)));
/// assert!("chunk-0".parse::<ReadMode>().is_err());
/// assert!("lines".parse::<ReadMode>().is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadMode {
    /// The whole input is one text (`all`, the default).
    All,
    /// The pipeline runs once per line (`line`).
    Line,
    /// The pipeline runs once per piece of this many characters (`chunk-N`).
    Chunk(usize),
}

impl FromStr for ReadMode {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text {
            "all" => Ok(ReadMode::All),
            "line" => Ok(ReadMode::Line),
            _ => match text.strip_prefix("chunk-") {
                Some(number) => match number.parse::<usize>() {
                    Ok(0) => Err("the chunk size must be at least 1 character".to_owned()),
                    Ok(size) => Ok(ReadMode::Chunk(size)),
                    Err(_) => Err(
                        "the chunk size after `chunk-` must be a whole number of characters, as in chunk-4096"
                            .to_owned(),
                    ),
                },
                None => Err(
                    "unknown read mode; use all, line or chunk-N (for example chunk-4096)"
                        .to_owned(),
                ),
            },
        }
    }
}

/// Runs pipeline `identifier` on `input` the way `txf` does.
///
/// `input_file` is the file the input was read from, exposed to the pipeline as
/// `__INPUT_FILE__` (empty when `None`). With `debug`, the step-by-step trace of
/// every run is written there; nothing is written for a run that fails.
///
/// ```
/// use textforge::api::{TextForgeBuilderMethods, processor::TextForgeProcessor};
/// use textforge::utils::cli::{ReadMode, process_input};
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().add_to_beginning("> ")?.build()?;
/// let output = process_input(&mut processor, &id, "a\nb\n".to_owned(), ReadMode::Line, None, None)?;
/// assert_eq!(output, "> a\n> b\n");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
pub fn process_input(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: String,
    read_mode: ReadMode,
    input_file: Option<&Path>,
    mut debug: Option<&mut (dyn Write + '_)>,
) -> Result<String, TextForgeError> {
    match read_mode {
        // An empty input still runs the pipeline: `atb`, `apl`, `ppf` and others
        // produce text from nothing. Before 0.3.2 it returned an empty result.
        ReadMode::All => run_piece(
            processor,
            identifier,
            Cow::Owned(input),
            input_file,
            debug.as_deref_mut(),
        ),
        ReadMode::Line => {
            // Each line is processed without its line break, which is written
            // back as it was. Lines follow `docs/lines.md`: LF, CRLF, CR, VT,
            // FF, NEL, LS and PS end a line, as for `__LINES__` and the line
            // instructions. Before 0.3.2 only LF (and CRLF) did.
            let mut output = String::with_capacity(input.len());
            for (index, (content, line_break)) in crate::utils::lines::lines(&input).enumerate() {
                announce(debug.as_deref_mut(), "line", index)?;
                let processed = run_piece(
                    processor,
                    identifier,
                    Cow::Borrowed(content),
                    input_file,
                    debug.as_deref_mut(),
                )
                .map_err(|mut e| {
                    e.instruction = format!("input line {} › {}", index + 1, e.instruction).into();
                    e
                })?;
                output.push_str(&processed);
                output.push_str(line_break);
            }
            Ok(output)
        }
        ReadMode::Chunk(size) => {
            if size == 0 {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::ZeroDivisionError(
                        "the chunk size must be at least 1 character".into(),
                    ),
                    Cow::Borrowed("txf"),
                    crate::utils::errors::input_excerpt(&input),
                ));
            }
            let mut output = String::with_capacity(input.len());
            for (index, chunk) in chunks(&input, size).enumerate() {
                announce(debug.as_deref_mut(), "chunk", index)?;
                let processed = run_piece(
                    processor,
                    identifier,
                    Cow::Borrowed(chunk),
                    input_file,
                    debug.as_deref_mut(),
                )
                .map_err(|mut e| {
                    e.instruction = format!("input chunk {} › {}", index + 1, e.instruction).into();
                    e
                })?;
                output.push_str(&processed);
            }
            Ok(output)
        }
    }
}

/// The two forms of a pipeline file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineFormat {
    /// A `.textforge` (`.tft`) text pipeline.
    Text,
    /// A `.textforgebc` (`.tfb`) bytecode pipeline.
    Bytecode,
}

/// The format of the pipeline file at `path`, read from its first bytes: a
/// file that starts with the bytecode magic number is bytecode, anything else
/// is text. `ppjn` tells them apart the same way.
///
/// ```
/// use textforge::utils::cli::{PipelineFormat, detect_pipeline_format};
///
/// let path = std::env::temp_dir().join(format!("detect-{}.textforge", std::process::id()));
/// std::fs::write(&path, "tua;\n")?;
/// assert_eq!(detect_pipeline_format(&path)?, PipelineFormat::Text);
/// std::fs::remove_file(&path)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn detect_pipeline_format(path: &Path) -> Result<PipelineFormat, TextForgeError> {
    use std::io::Read;
    let error = |code: TextForgeErrorCode| {
        TextForgeError::new(code, Cow::Borrowed("txf"), path.display().to_string())
    };
    let file = std::fs::File::open(path).map_err(|e| {
        error(TextForgeErrorCode::FileOpeningError(
            format!("cannot open the pipeline file `{}`: {e}", path.display()).into(),
        ))
    })?;
    let mut start = Vec::with_capacity(crate::parser::joins::BYTECODE_MAGIC.len());
    file.take(crate::parser::joins::BYTECODE_MAGIC.len() as u64)
        .read_to_end(&mut start)
        .map_err(|e| {
            error(TextForgeErrorCode::FileReadingError(
                format!("cannot read the pipeline file `{}`: {e}", path.display()).into(),
            ))
        })?;
    Ok(if start == crate::parser::joins::BYTECODE_MAGIC {
        PipelineFormat::Bytecode
    } else {
        PipelineFormat::Text
    })
}

/// Loads the pipeline file at `path` into `processor` and returns its id.
///
/// With `format` set to `None` the format is detected with
/// [`detect_pipeline_format`] and any pipeline extension is accepted
/// (`.textforge`, `.tft`, `.textforgebc`, `.tfb`); with a format, the file must
/// have one of that format's extensions.
///
/// ```
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
/// use textforge::utils::cli::load_pipeline;
///
/// let path = std::env::temp_dir().join(format!("load-{}.tft", std::process::id()));
/// std::fs::write(&path, "tua;\n")?;
/// let mut processor = TextForgeProcessor::new();
/// let id = load_pipeline(&mut processor, &path, None)?;
/// assert_eq!(processor.process_all(&id, "abc")?, "ABC");
/// std::fs::remove_file(&path)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_pipeline(
    processor: &mut TextForgeProcessor,
    path: &Path,
    format: Option<PipelineFormat>,
) -> Result<String, TextForgeError> {
    use crate::{api::processor::TextForgeProcessorMethods, utils::validations::check_file_path};
    let format = match format {
        Some(format) => {
            let extension = match format {
                PipelineFormat::Text => "textforge",
                PipelineFormat::Bytecode => "textforgebc",
            };
            check_file_path(path, Some(extension))?;
            format
        }
        None => {
            check_file_path(path, None)?;
            detect_pipeline_format(path)?
        }
    };
    match format {
        PipelineFormat::Text => processor.get_pipeline_from_file(path),
        // The `cli` feature turns on `bytecode`.
        PipelineFormat::Bytecode => processor.read_from_bytecode_file(path),
    }
}

/// The built-in metrics `txf --watch` accepts, in report order.
///
/// ```
/// use textforge::utils::cli::watch_metrics;
///
/// assert!(watch_metrics().contains(&"line_count"));
/// ```
pub fn watch_metrics() -> Vec<&'static str> {
    crate::api::portable::watchers::BUILTINS
        .iter()
        .map(|(name, _)| *name)
        .collect()
}

/// Runs pipeline `identifier` on `input` with the built-in watchers `metrics`
/// (names from [`watch_metrics`], or `all`) after every instruction, and writes
/// their JSON report to `report`. `input_file` is exposed as `__INPUT_FILE__`.
///
/// ```
/// use textforge::api::{TextForgeBuilderMethods, processor::TextForgeProcessor};
/// use textforge::utils::cli::process_with_watchers;
///
/// let mut processor = TextForgeProcessor::new();
/// let id = processor.create_pipeline().to_uppercase_all()?.build()?;
/// let report = std::env::temp_dir().join(format!("watch-{}.json", std::process::id()));
/// let output = process_with_watchers(&mut processor, &id, "ab", None, &["current_len_chars"], &report)?;
/// assert_eq!(output, "AB");
/// assert!(std::fs::read_to_string(&report)?.contains("current_len_chars"));
/// std::fs::remove_file(&report)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn process_with_watchers(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    input_file: Option<&Path>,
    metrics: &[&str],
    report: &Path,
) -> Result<String, TextForgeError> {
    use crate::{
        api::portable::watchers::BUILTINS,
        watchers::{ExecutionWindows, WatcherList},
    };
    let mut watchers = WatcherList::new();
    let wanted: Vec<&str> = if metrics.contains(&"all") {
        watch_metrics()
    } else {
        metrics.to_vec()
    };
    for metric in wanted {
        let (_, callback) = BUILTINS
            .iter()
            .find(|(name, _)| *name == metric)
            .ok_or_else(|| {
                TextForgeError::new(
                    TextForgeErrorCode::WatcherNotFoundError(
                        format!(
                            "unknown metric `{metric}`; use all or one of: {}",
                            watch_metrics().join(", ")
                        )
                        .into(),
                    ),
                    Cow::Borrowed("--watch"),
                    metric.to_owned(),
                )
            })?;
        let callback = *callback;
        watchers.set_owned_watcher(metric.to_owned(), move |context| Ok(callback(context)));
        watchers.schedule_owned_watcher(metric, ExecutionWindows::All)?;
    }
    processor.process_watchers_from(identifier, input, input_file, &mut watchers, report)
}

/// In a trace of `line` or `chunk` mode, the piece the next run belongs to
/// (`=== input line 2 ===`). Before 0.3.2 the traces followed one another
/// with nothing between them.
fn announce(
    debug: Option<&mut (dyn Write + '_)>,
    piece: &str,
    index: usize,
) -> Result<(), TextForgeError> {
    let Some(log) = debug else {
        return Ok(());
    };
    writeln!(log, "=== input {piece} {} ===", index + 1).map_err(|error| {
        TextForgeError::new(
            TextForgeErrorCode::FileWritingError(
                format!("cannot write the debug log: {error}").into(),
            ),
            "txf",
            "",
        )
    })
}

/// One run of the pipeline, with or without a trace.
fn run_piece(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    piece: Cow<'_, str>,
    input_file: Option<&Path>,
    debug: Option<&mut (dyn Write + '_)>,
) -> Result<String, TextForgeError> {
    match debug {
        Some(log) => processor.process_debug_from(identifier, &piece, input_file, log),
        None => processor.process_text_from(identifier, piece, input_file),
    }
}

/// [`process_input`] with the trace, if any, on standard output.
fn process_to_stdout(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    read_mode: ReadMode,
    debug: bool,
) -> Result<String, TextForgeError> {
    let mut stdout = std::io::stdout().lock();
    let log = debug.then_some(&mut stdout as &mut dyn Write);
    process_input(
        processor,
        identifier,
        input.to_owned(),
        read_mode,
        None,
        log,
    )
}

/// Runs a pipeline on the whole input at once (`--read-mode all`), printing
/// the trace with `debug`. See [`process_input`] to pass the input file.
pub fn process_input_single_chunk(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    debug: bool,
) -> Result<String, TextForgeError> {
    process_to_stdout(processor, identifier, input, ReadMode::All, debug)
}

/// `input` without its final line break, and that line break.
///
/// A line break is any terminator of `docs/lines.md`: LF, CRLF (taken whole),
/// CR, VT, FF, NEL, LINE SEPARATOR or PARAGRAPH SEPARATOR. Up to 0.3.2 only LF
/// and CRLF were recognized.
///
/// Files usually end with a line break, which is a character like any other to
/// the instructions: `dll` would delete it and `ate` would write after it.
/// `txf --chomp` sets it aside with this function and puts it back on the
/// result.
///
/// ```
/// use textforge::utils::cli::split_final_newline;
///
/// assert_eq!(split_final_newline("abc\r\n"), ("abc", "\r\n"));
/// assert_eq!(split_final_newline("a\n\n"), ("a\n", "\n"));
/// assert_eq!(split_final_newline("abc"), ("abc", ""));
/// assert_eq!(split_final_newline("abc\r"), ("abc", "\r"));
/// assert_eq!(split_final_newline("abc\u{2028}"), ("abc", "\u{2028}"));
/// assert_eq!(split_final_newline(""), ("", ""));
/// ```
pub fn split_final_newline(input: &str) -> (&str, &str) {
    let Some(last) = input.chars().next_back() else {
        return (input, "");
    };
    if !crate::utils::lines::is_break(last) {
        return (input, "");
    }
    let mut body = &input[..input.len() - last.len_utf8()];
    if last == '\n' {
        body = body.strip_suffix('\r').unwrap_or(body);
    }
    input.split_at(body.len())
}

/// Runs a pipeline on each line of the input (`--read-mode line`), printing
/// the trace with `debug`. See [`process_input`] to pass the input file.
pub fn process_input_line_by_line(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    debug: bool,
) -> Result<String, TextForgeError> {
    process_to_stdout(processor, identifier, input, ReadMode::Line, debug)
}

/// Runs a pipeline on consecutive chunks of `chunk_size` characters
/// (`--read-mode chunk-N`), printing the trace with `debug`. See
/// [`process_input`] to pass the input file.
pub fn process_input_by_chunks(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    chunk_size: usize,
    debug: bool,
) -> Result<String, TextForgeError> {
    if input.is_empty() {
        return Ok(String::new());
    }
    process_to_stdout(
        processor,
        identifier,
        input,
        ReadMode::Chunk(chunk_size),
        debug,
    )
}

/// `input` cut into consecutive pieces of `size` characters (the last one may
/// be shorter), in one forward pass. A piece never ends between the `\r` and
/// the `\n` of a CRLF: it takes the `\n` too, so it has `size + 1` characters.
/// Up to 0.3.2 every boundary was searched from the start of the text, which
/// made chunk mode quadratic, and a CRLF could be split.
fn chunks(input: &str, size: usize) -> impl Iterator<Item = &str> {
    let mut rest = input;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        let mut end = rest.char_indices().nth(size).map_or(rest.len(), |(i, _)| i);
        if rest[..end].ends_with('\r') && rest[end..].starts_with('\n') {
            end += 1;
        }
        let (chunk, tail) = rest.split_at(end);
        rest = tail;
        Some(chunk)
    })
}

#[cfg(feature = "test_access")]
#[cfg(test)]
mod cli_tests {
    mod process_input_by_chunks_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_by_chunks, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha";
            let expected_output = "bcolbxilbnhlbal".to_string();
            let result = process_input_by_chunks(&mut processor, &id, input, 2, true)?;

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }

    mod chunks_tests {
        use crate::utils::cli::chunks;

        #[test]
        fn chunks_count_characters_and_keep_every_byte() {
            let cut = |text, size| chunks(text, size).collect::<Vec<_>>();
            assert_eq!(cut("abcdefg", 3), ["abc", "def", "g"]);
            assert_eq!(cut("abc", 3), ["abc"]);
            assert_eq!(cut("abc", 10), ["abc"]);
            assert_eq!(cut("é😀ab", 1), ["é", "😀", "a", "b"]);
            assert_eq!(cut("e\u{301}x", 1), ["e", "\u{301}", "x"]);
            // A CRLF stays whole (before: "ab\r" and "\ncd").
            assert_eq!(cut("ab\r\ncd", 3), ["ab\r\n", "cd"]);
            assert_eq!(cut("\r\n\r\n", 1), ["\r\n", "\r\n"]);
            assert_eq!(cut("a\rb", 2), ["a\r", "b"]);
            assert!(cut("", 4).is_empty());
            // One pass: a long text with small chunks is cut in linear time.
            let long = "ab".repeat(200_000);
            assert_eq!(chunks(&long, 1).count(), 400_000);
        }
    }

    mod process_input_line_by_line_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_line_by_line, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha\nlaranja";
            let expected_output = "bcoxinhal\nblaranjal".to_string();
            let result = process_input_line_by_line(&mut processor, &id, input, true)?;

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }

    mod regression_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{
                cli::{process_input_line_by_line, process_input_single_chunk},
                errors::TextForgeError,
            },
        };

        /// Before 0.3.2 an empty input skipped the pipeline.
        #[test]
        fn an_empty_input_still_runs_the_pipeline() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();
            let id = processor.create_pipeline().add_to_beginning("X")?.build()?;
            assert_eq!(
                process_input_single_chunk(&mut processor, &id, "", false)?,
                "X"
            );
            assert_eq!(
                process_input_line_by_line(&mut processor, &id, "", false)?,
                ""
            );
            Ok(())
        }

        /// Before 0.3.2 line mode turned `\r\n` into `\n` and dropped the last
        /// line break; CR and the Unicode terminators did not end a line.
        #[test]
        fn line_mode_keeps_every_line_break() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();
            let id = processor.create_pipeline().add_to_beginning("X")?.build()?;
            for (input, expected) in [
                ("a\r\nb\r\n", "Xa\r\nXb\r\n"),
                ("a\nb", "Xa\nXb"),
                ("a\n\nb\n", "Xa\nX\nXb\n"),
                ("a\r\nb\n", "Xa\r\nXb\n"),
                ("a\rb\rc", "Xa\rXb\rXc"),
                ("a\u{2028}b\u{85}c\u{c}", "Xa\u{2028}Xb\u{85}Xc\u{c}"),
                ("a\r\r\nb", "Xa\rX\r\nXb"),
            ] {
                assert_eq!(
                    process_input_line_by_line(&mut processor, &id, input, false)?,
                    expected
                );
            }
            Ok(())
        }

        #[test]
        fn line_mode_errors_name_the_input_line() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();
            let id = processor.create_pipeline().delete_single(2)?.build()?;
            let error =
                process_input_line_by_line(&mut processor, &id, "abc\nab\n", false).unwrap_err();
            assert!(
                error.instruction.starts_with("input line 2 › "),
                "{error:?}"
            );
            Ok(())
        }
    }

    mod process_input_single_chunk_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_single_chunk, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha";
            let expected_output = "bcoxinhal".to_string();
            let result = process_input_single_chunk(&mut processor, &id, input, true)?;

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }
}

/// Where a value of the effective profile came from (`txf config show`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSource {
    /// The `balanced` default.
    Default,
    /// A preset chosen elsewhere (its name).
    Preset(String),
    /// A configuration file.
    File(std::path::PathBuf),
    /// A command-line flag.
    Flag,
}

impl std::fmt::Display for ProfileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileSource::Default => f.write_str("default"),
            ProfileSource::Preset(name) => write!(f, "preset {name}"),
            ProfileSource::File(path) => write!(f, "file {}", path.display()),
            ProfileSource::Flag => f.write_str("command-line flag"),
        }
    }
}

/// Profile-related `txf` flags.
#[derive(Clone, Debug, Default)]
pub struct ProfileOptions {
    /// `--profile`
    pub profile: Option<String>,
    /// `--spool-threshold`
    pub spool_threshold: Option<String>,
    /// `--max-generated-bytes` (a size or `unlimited`)
    pub max_generated_bytes: Option<String>,
    /// `--max-iterations` (a number or `unlimited`)
    pub max_iterations: Option<String>,
    /// `--config`
    pub config: Option<std::path::PathBuf>,
    /// `--no-rc`
    pub no_rc: bool,
}

/// The effective profile and the origin of each value.
#[derive(Clone, Debug)]
pub struct ResolvedProfile {
    /// The resulting profile.
    pub profile: crate::api::ExecutionProfile,
    /// The preset it starts from.
    pub preset: String,
    /// Where the preset came from.
    pub preset_source: ProfileSource,
    /// Where the spool threshold came from.
    pub spool_threshold_source: ProfileSource,
    /// Where the generated-bytes limit came from.
    pub max_generated_bytes_source: ProfileSource,
    /// Where the iteration limit came from.
    pub max_iterations_source: ProfileSource,
}

/// Resolve the `txf` profile. For each value, the strongest source wins:
/// command-line flags, then the file given with `--config`, then `.txfrc` in
/// `directory` (unless `--no-rc`), then the `balanced` preset. Only one file
/// is read; parent directories are not searched and files are never merged.
pub fn resolve_profile(
    options: &ProfileOptions,
    directory: &std::path::Path,
) -> Result<ResolvedProfile, TextForgeError> {
    use crate::api::{ExecutionProfile, RcFile, profile::parse_size};

    let file = match (&options.config, options.no_rc) {
        (Some(path), _) => Some(path.clone()),
        (None, false) => Some(directory.join(".txfrc")).filter(|path| path.is_file()),
        (None, true) => None,
    };
    let rc = file.as_deref().map(RcFile::from_path).transpose()?;

    let (preset, preset_source) = match (&options.profile, rc.as_ref().and_then(RcFile::preset)) {
        (Some(name), _) => (name.clone(), ProfileSource::Flag),
        (None, Some(name)) => (
            name.to_owned(),
            ProfileSource::File(file.clone().unwrap_or_default()),
        ),
        (None, None) => ("balanced".to_owned(), ProfileSource::Default),
    };
    let mut profile = ExecutionProfile::preset(&preset)?;
    let mut spool_threshold_source = ProfileSource::Preset(preset.clone());
    if let Some(threshold) = rc
        .as_ref()
        .map(RcFile::spool_threshold)
        .transpose()?
        .flatten()
    {
        profile.spool_threshold = threshold;
        spool_threshold_source = ProfileSource::File(file.clone().unwrap_or_default());
    }
    if let Some(text) = &options.spool_threshold {
        profile.spool_threshold = parse_size(text).map_err(from_flag("--spool-threshold"))?;
        spool_threshold_source = ProfileSource::Flag;
    }

    // Limits: flag > file > default (the same in every preset).
    let file_source = || ProfileSource::File(file.clone().unwrap_or_default());
    let from_file = match &rc {
        Some(rc) => rc.limits()?,
        None => Default::default(),
    };
    let limit = |flag: &Option<String>,
                 name: &'static str,
                 parse: fn(&str) -> Result<Option<u64>, TextForgeError>,
                 from_file: Option<Option<u64>>,
                 target: &mut Option<u64>|
     -> Result<ProfileSource, TextForgeError> {
        if let Some(text) = flag {
            *target = parse(text).map_err(from_flag(name))?;
            Ok(ProfileSource::Flag)
        } else if let Some(value) = from_file {
            *target = value;
            Ok(file_source())
        } else {
            Ok(ProfileSource::Default)
        }
    };
    let max_generated_bytes_source = limit(
        &options.max_generated_bytes,
        "--max-generated-bytes",
        crate::api::profile::parse_limit,
        from_file.max_generated_bytes,
        &mut profile.limits.max_generated_bytes,
    )?;
    let max_iterations_source = limit(
        &options.max_iterations,
        "--max-iterations",
        crate::api::profile::parse_count,
        from_file.max_iterations,
        &mut profile.limits.max_iterations,
    )?;
    Ok(ResolvedProfile {
        profile,
        preset,
        preset_source,
        spool_threshold_source,
        max_generated_bytes_source,
        max_iterations_source,
    })
}

/// Errors of a flag name the flag (`in: --spool-threshold`), not the
/// configuration file. Before 0.3.2 they said `in: txfrc`.
fn from_flag(flag: &'static str) -> impl Fn(TextForgeError) -> TextForgeError {
    move |mut error| {
        error.instruction = Cow::Borrowed(flag);
        error
    }
}

impl ResolvedProfile {
    /// The text `txf config show` prints.
    pub fn describe(&self) -> String {
        let show = |limit: Option<u64>, unit: &str| {
            limit.map_or_else(|| "unlimited".to_owned(), |value| format!("{value}{unit}"))
        };
        let mut text = format!(
            "profile: {} ({})\nspool_threshold: {} bytes ({})\nmax_generated_bytes: {} ({})\nmax_iterations: {} ({})\n",
            self.preset,
            self.preset_source,
            self.profile.spool_threshold,
            self.spool_threshold_source,
            show(self.profile.limits.max_generated_bytes, " bytes"),
            self.max_generated_bytes_source,
            show(self.profile.limits.max_iterations, ""),
            self.max_iterations_source
        );
        if !cfg!(feature = "buffered") {
            text.push_str(
                "note: this txf was built without the `buffered` feature; spool_threshold has no effect\n",
            );
        }
        text
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use crate::api::ExecutionProfile;
    use std::path::PathBuf;

    struct Directory(PathBuf);
    impl Directory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("txf-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Directory(path)
        }
        fn rc(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn without_files_or_flags_the_profile_is_balanced() {
        let directory = Directory::new("none");
        let resolved = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap();
        assert_eq!(resolved.profile, ExecutionProfile::balanced());
        assert_eq!(resolved.preset_source, ProfileSource::Default);
        assert!(
            resolved
                .describe()
                .starts_with("profile: balanced (default)\n")
        );
    }

    #[test]
    fn flags_beat_the_config_file_which_beats_the_local_rc() {
        let directory = Directory::new("precedence");
        let local = directory.rc(
            ".txfrc",
            r#"{"version": 1, "profile": "low-memory", "overrides": {"spool_threshold": "8KiB"}}"#,
        );
        let explicit = directory.rc("other.json", r#"{"version": 1, "profile": "fast"}"#);

        let resolved = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap();
        assert_eq!(resolved.preset, "low-memory");
        assert_eq!(resolved.profile.spool_threshold, 8192);
        assert_eq!(
            resolved.spool_threshold_source,
            ProfileSource::File(local.clone())
        );

        // --config replaces the local file entirely (no merging).
        let options = ProfileOptions {
            config: Some(explicit.clone()),
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.profile, ExecutionProfile::fast());
        assert_eq!(resolved.preset_source, ProfileSource::File(explicit));

        // Flags win, value by value.
        let options = ProfileOptions {
            profile: Some("fast".into()),
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.preset_source, ProfileSource::Flag);
        assert_eq!(resolved.profile.spool_threshold, 8192, "file override kept");
        let options = ProfileOptions {
            spool_threshold: Some("2MiB".into()),
            no_rc: true,
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.preset, "balanced");
        assert_eq!(resolved.profile.spool_threshold, 2 * 1024 * 1024);
        assert_eq!(resolved.spool_threshold_source, ProfileSource::Flag);
        assert!(resolved.describe().contains("(command-line flag)"));
    }

    #[test]
    fn limits_come_from_flags_then_the_file_then_the_defaults() {
        let directory = Directory::new("limits");
        let local = directory.rc(
            ".txfrc",
            r#"{"version": 1, "overrides": {"max_generated_bytes": "unlimited", "max_iterations": 7}}"#,
        );
        let resolved = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap();
        assert_eq!(resolved.profile.limits.max_generated_bytes, None);
        assert_eq!(resolved.profile.limits.max_iterations, Some(7));
        assert_eq!(resolved.max_iterations_source, ProfileSource::File(local));
        assert!(
            resolved
                .describe()
                .contains("max_generated_bytes: unlimited")
        );

        let options = ProfileOptions {
            max_generated_bytes: Some("16GiB".into()),
            max_iterations: Some("unlimited".into()),
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.profile.limits.max_generated_bytes, Some(16 << 30));
        assert_eq!(resolved.profile.limits.max_iterations, None);
        assert_eq!(resolved.max_generated_bytes_source, ProfileSource::Flag);

        let options = ProfileOptions {
            no_rc: true,
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.profile.limits, crate::api::Limits::default());
        assert_eq!(resolved.max_iterations_source, ProfileSource::Default);
        let options = ProfileOptions {
            max_iterations: Some("many".into()),
            no_rc: true,
            ..Default::default()
        };
        assert!(resolve_profile(&options, &directory.0).is_err());
    }

    /// Before: an invalid flag value was reported as `in: txfrc`.
    #[test]
    fn flag_errors_name_the_flag() {
        let directory = Directory::new("flag-errors");
        for (options, flag) in [
            (
                ProfileOptions {
                    spool_threshold: Some("1".into()),
                    no_rc: true,
                    ..Default::default()
                },
                "--spool-threshold",
            ),
            (
                ProfileOptions {
                    max_iterations: Some("many".into()),
                    no_rc: true,
                    ..Default::default()
                },
                "--max-iterations",
            ),
            (
                ProfileOptions {
                    max_generated_bytes: Some("2 parsecs".into()),
                    no_rc: true,
                    ..Default::default()
                },
                "--max-generated-bytes",
            ),
        ] {
            let error = resolve_profile(&options, &directory.0).unwrap_err();
            assert_eq!(error.instruction, flag);
        }
        // A bad value in the file still names the file.
        directory.rc(
            ".txfrc",
            r#"{"version": 1, "overrides": {"max_iterations": "x"}}"#,
        );
        let error = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap_err();
        assert_eq!(error.instruction, "txfrc");
    }

    #[test]
    fn invalid_files_and_flags_are_errors() {
        let directory = Directory::new("invalid");
        directory.rc(".txfrc", r#"{"version": 1, "mystery": true}"#);
        assert!(resolve_profile(&ProfileOptions::default(), &directory.0).is_err());
        // --no-rc ignores it.
        let options = ProfileOptions {
            no_rc: true,
            ..Default::default()
        };
        assert!(resolve_profile(&options, &directory.0).is_ok());
        for options in [
            ProfileOptions {
                profile: Some("turbo".into()),
                no_rc: true,
                ..Default::default()
            },
            ProfileOptions {
                spool_threshold: Some("1".into()),
                no_rc: true,
                ..Default::default()
            },
            ProfileOptions {
                config: Some(directory.0.join("missing.json")),
                ..Default::default()
            },
        ] {
            assert!(resolve_profile(&options, &directory.0).is_err());
        }
    }
}
