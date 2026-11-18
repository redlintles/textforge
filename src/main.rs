//! `txf`, the TextForge command-line program: runs a `.textforge` or
//! `.textforgebc` pipeline on a file or standard input (`cli` feature).
use clap::{Arg, ArgAction, Command, value_parser};
use std::{
    borrow::Cow,
    fs::OpenOptions,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
};
use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    utils::{
        cli::{
            PipelineFormat, ProfileOptions, ReadMode, load_pipeline, process_input,
            process_with_watchers, resolve_profile, split_final_newline, watch_metrics,
            write_output,
        },
        errors::{TextForgeError, TextForgeErrorCode},
    },
};

/// Help heading of the profile and limit flags.
const PROFILE_HEADING: &str = "Profile and limits (never change the result)";

/// Flags choosing the execution profile, shared by runs and `config show`.
fn profile_args() -> [Arg; 6] {
    [
        Arg::new("profile")
            .long("profile")
            .value_name("PRESET")
            .value_parser(["balanced", "low-memory", "fast"])
            .hide_possible_values(true)
            .help_heading(PROFILE_HEADING)
            .help("Memory/speed preset: balanced (default), low-memory or fast"),
        Arg::new("spool_threshold")
            .long("spool-threshold")
            .value_name("SIZE")
            .help_heading(PROFILE_HEADING)
            .help("Text size kept in memory before spooling to disk, in bytes or with a unit (512MiB); only in builds with the buffered feature"),
        Arg::new("max_generated_bytes")
            .long("max-generated-bytes")
            .value_name("SIZE|unlimited")
            .help_heading(PROFILE_HEADING)
            .help("Largest text rpt, padl and padr may produce (default 2GiB)"),
        Arg::new("max_iterations")
            .long("max-iterations")
            .value_name("N|unlimited")
            .help_heading(PROFILE_HEADING)
            .help("Largest number of iter repetitions (default 10000000)"),
        Arg::new("config")
            .long("config")
            .value_name("FILE")
            .value_parser(value_parser!(PathBuf))
            .conflicts_with("no_rc")
            .help_heading(PROFILE_HEADING)
            .help("Read this configuration file instead of .txfrc in the current directory"),
        Arg::new("no_rc")
            .long("no-rc")
            .action(ArgAction::SetTrue)
            .help_heading(PROFILE_HEADING)
            .help("Do not read .txfrc"),
    ]
}

/// The profile flags of `matches`. For `config show`, `outer` holds the flags
/// written before the subcommand; those written after it win.
fn profile_options(matches: &clap::ArgMatches) -> ProfileOptions {
    profile_options_of(&[matches])
}

fn profile_options_of(levels: &[&clap::ArgMatches]) -> ProfileOptions {
    let text = |name: &str| {
        levels
            .iter()
            .find_map(|matches| matches.get_one::<String>(name).cloned())
    };
    ProfileOptions {
        profile: text("profile"),
        spool_threshold: text("spool_threshold"),
        max_generated_bytes: text("max_generated_bytes"),
        max_iterations: text("max_iterations"),
        config: levels
            .iter()
            .find_map(|matches| matches.get_one::<PathBuf>("config").cloned()),
        no_rc: levels.iter().any(|matches| matches.get_flag("no_rc")),
    }
}

/// The pipeline file a subcommand works on.
fn pipeline_arg(help: &'static str) -> Arg {
    Arg::new("pipeline")
        .value_name("PIPELINE")
        .required(true)
        .value_parser(value_parser!(PathBuf))
        .help(help)
}

/// The file a subcommand writes.
fn output_arg(help: &'static str) -> Arg {
    Arg::new("output")
        .short('o')
        .long("output")
        .value_name("FILE")
        .value_parser(value_parser!(PathBuf))
        .help(help)
}

fn build_cli() -> Command {
    Command::new("txf")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Run TextForge text-processing pipelines on a file or on standard input")
        .long_about("Run a TextForge pipeline on a file or on standard input and print or save the result. A pipeline is a list of instructions, one per line, each ending with ; (tbs; rfw world Rust; tua;). Text pipelines are .textforge or .tft files; bytecode pipelines, made with `txf compile`, are .textforgebc or .tfb files.")
        .after_help(concat!(
            "Exit status: 0 success, 1 the pipeline failed on the text, 2 invalid arguments or configuration, 3 the pipeline could not be loaded, 4 the input could not be read or the result written.\n",
            "Run `txf --help` for examples."
        ))
        .after_long_help(EXIT_STATUS_HELP)
        .subcommand_negates_reqs(true)
        .subcommand(
            Command::new("compile")
                .about("Compile a text pipeline to bytecode")
                .long_about("Compile a text pipeline (.textforge, .tft) to bytecode. Joined pipelines (ppjn) are embedded, so the bytecode runs without their files.")
                .arg(pipeline_arg("The text pipeline to compile"))
                .arg(output_arg("The bytecode file to write (.textforgebc or .tfb); default: the pipeline's name with .textforgebc, next to it")),
        )
        .subcommand(
            Command::new("decompile")
                .about("Write a bytecode pipeline back as text")
                .arg(pipeline_arg("The bytecode pipeline to read"))
                .arg(output_arg("The text file to write (.textforge or .tft); default: the pipeline's name with .textforge, next to it")),
        )
        .subcommand(
            Command::new("check")
                .about("Load pipelines without running them and report problems")
                .long_about("Load pipelines without running them: parse them, validate every instruction and load joined pipelines. Prints one line per pipeline; the exit status is 3 when any fails.")
                .arg(
                    Arg::new("pipelines")
                        .value_name("PIPELINE")
                        .required(true)
                        .num_args(1..)
                        .value_parser(value_parser!(PathBuf))
                        .help("Pipeline files, text or bytecode"),
                ),
        )
        .subcommand(
            Command::new("config")
                .about("Inspect the configuration: .txfrc and the profile flags")
                .subcommand_required(true)
                .subcommand(
                    Command::new("show")
                        .about("Print the effective profile and where each value comes from")
                        .args(profile_args()),
                ),
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .required(true)
                .value_name("PIPELINE")
                .value_parser(value_parser!(PathBuf))
                .help_heading("Pipeline")
                .help("The pipeline to run: text (.textforge, .tft) or bytecode (.textforgebc, .tfb)")
        )
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .required(false)
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .help_heading("Input and output")
                .help(
                    "The UTF-8 text to process; without it, or with -, txf reads standard input. The pipeline sees this path as __INPUT_FILE__"
                )
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .required(false)
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .help_heading("Input and output")
                .help(
                    "Where to write the result (replaced atomically); without it, or with -, txf prints the result"
                )
        )
        .arg(
            Arg::new("read_mode")
                .short('r')
                .long("read-mode")
                .default_value("all")
                .required(false)
                .value_name("MODE")
                .value_parser(|s: &str| s.parse::<ReadMode>())
                .help_heading("Input and output")
                .help(
                    "How to feed the input to the pipeline: 'all' runs it once on the whole text; 'line' once per line, without its line break (kept as it was); 'chunk-N' once per piece of N characters. line and chunk-N run nothing on an empty input"
                )
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .required(false)
                .value_name("MODE")
                .value_parser(["auto", "t", "text", "b", "bytecode"])
                .default_value("auto")
                .hide_possible_values(true)
                .help_heading("Pipeline")
                .help(
                    "How to read --file: 'auto' (default) tells text from bytecode by the first bytes of the file; 't'/'text' or 'b'/'bytecode' force a format and check its extension"
                )
        )
        .arg(
            Arg::new("chomp")
                .long("chomp")
                .required(false)
                .action(ArgAction::SetTrue)
                .help_heading("Input and output")
                .help(
                    "Set the final line break of the input aside and put it back on the result, so instructions such as dll and ate work on the last visible character (--read-mode line always does this for every line)"
                )
        )
        .arg(
            Arg::new("watch")
                .long("watch")
                .value_name("METRIC")
                .value_delimiter(',')
                .value_parser(clap::builder::PossibleValuesParser::new(
                    std::iter::once("all").chain(watch_metrics()),
                ))
                .hide_possible_values(true)
                .action(ArgAction::Append)
                .requires("watch_report")
                .conflicts_with("debug")
                .help_heading("Diagnostics")
                .help(format!(
                    "Record a metric after every instruction into the --watch-report file (repeatable or comma-separated; 'all' for every metric): {}",
                    watch_metrics().join(", ")
                )),
        )
        .arg(
            Arg::new("watch_report")
                .long("watch-report")
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .requires("watch")
                .help_heading("Diagnostics")
                .help("Where --watch writes its JSON report (one row per instruction); needs --read-mode all"),
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .required(false)
                .action(ArgAction::SetTrue)
                .help_heading("Diagnostics")
                .help(
                    "Print the step-by-step trace of the run instead of the result; with --output the result is still written to the file"
                )
        )
        .args(profile_args())
}

/// Standard output that remembers when its reader has gone away.
///
/// Writing to a closed pipe (`txf … | head`) fails with `BrokenPipe`. That is
/// not an error of the run: the reader got what it wanted, so `txf` stops and
/// exits with 0, like other Unix tools. Before 0.3.2, `print!` panicked.
struct Stdout {
    inner: io::StdoutLock<'static>,
    closed: bool,
}

impl Stdout {
    fn new() -> Self {
        Stdout {
            inner: io::stdout().lock(),
            closed: false,
        }
    }

    fn note(&mut self, error: io::Error) -> io::Error {
        if error.kind() == io::ErrorKind::BrokenPipe {
            self.closed = true;
        }
        error
    }
}

impl Write for Stdout {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes).map_err(|error| self.note(error))
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush().map_err(|error| self.note(error))
    }
}

/// The exit status says which step failed, so scripts can tell a pipeline
/// that rejected its text from a typing mistake or a missing file. Up to
/// 0.3.2 every failure exited with 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// The pipeline failed on the text (also errors of `wtf`, `apf` and the
    /// other file instructions while running).
    Run = 1,
    /// Invalid arguments or configuration (`.txfrc`, profile flags). clap
    /// exits with this status for its own errors.
    Usage = 2,
    /// The pipeline file could not be read, parsed or validated.
    Load = 3,
    /// The input could not be read or the result could not be written.
    Io = 4,
}

/// An error and the step it happened in.
struct Failure {
    stage: Stage,
    error: TextForgeError,
}

/// `map_err` adapter that tags an error with its step.
fn at(stage: Stage) -> impl Fn(TextForgeError) -> Failure {
    move |error| Failure { stage, error }
}

/// The text of `txf --help` after the options.
const EXIT_STATUS_HELP: &str = "\
Examples:
  txf -f clean.textforge -i in.txt -o out.txt   run a pipeline on a file
  cat in.txt | txf -f clean.tft                 read stdin, print the result
  txf -f trim.tft -r line -i log.txt            run it on every line
  txf -f clean.tft -i in.txt -d                 show each step instead of the result
  txf compile clean.textforge                   write clean.textforgebc
  txf check *.textforge                         load pipelines without running them
  txf config show                               print the effective profile

Exit status:
  0  success
  1  the pipeline failed on the text
  2  invalid arguments or configuration (.txfrc, profile flags)
  3  the pipeline could not be loaded (missing file, syntax, validation)
  4  the input could not be read or the result could not be written

Guide: https://github.com/redlintles/textforge/blob/main/docs/cli.md";

/// Runs the program and prints an error the way a person reads it
/// (`error[12]: message`, then the instruction and the input) on standard
/// error. Before 0.3.2 the error was printed as a Rust debug structure.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(Failure { stage, error }) => {
            eprintln!("{error}");
            std::process::ExitCode::from(stage as u8)
        }
    }
}

fn run() -> Result<(), Failure> {
    let matches = build_cli().get_matches();
    let mut stdout = Stdout::new();
    let profile_of = |options: ProfileOptions| {
        resolve_profile(&options, Path::new(".")).map_err(at(Stage::Usage))
    };
    match matches.subcommand() {
        Some(("config", config)) => {
            let show = config.subcommand_matches("show").unwrap_or(config);
            let profile = profile_of(profile_options_of(&[show, &matches]))?;
            return print_result(&mut stdout, &profile.describe()).map_err(at(Stage::Io));
        }
        Some(("compile", sub)) => return convert(sub, PipelineFormat::Text, &mut stdout),
        Some(("decompile", sub)) => return convert(sub, PipelineFormat::Bytecode, &mut stdout),
        Some(("check", sub)) => return check(sub, &mut stdout),
        _ => {}
    }
    let profile = profile_of(profile_options(&matches))?;

    let file = argument::<PathBuf>(&matches, "file").map_err(at(Stage::Usage))?;
    let input = matches.get_one::<PathBuf>("input");
    let output = matches.get_one::<PathBuf>("output");
    let textforge_mode = argument::<String>(&matches, "mode").map_err(at(Stage::Usage))?;
    let read_mode = argument::<ReadMode>(&matches, "read_mode").map_err(at(Stage::Usage))?;
    let debug = matches.get_flag("debug");

    // `-` is standard input for --input and standard output for --output.
    let input = input.filter(|path| !is_dash(path));
    let output = output.filter(|path| !is_dash(path));

    // Load the pipeline first, so a mistake in it is reported at once instead
    // of after the whole input has been read. Before 0.3.2 the input came first.
    let format = match textforge_mode.as_str() {
        "t" | "text" => Some(PipelineFormat::Text),
        "b" | "bytecode" => Some(PipelineFormat::Bytecode),
        _ => None,
    };
    let mut processor = TextForgeProcessor::new();
    let id = load_pipeline(&mut processor, file, format).map_err(at(Stage::Load))?;
    processor.set_profile(profile.profile);
    if input.is_none() && io::stdin().is_terminal() {
        eprintln!(
            "txf: reading the text from standard input; end it with Ctrl-D (Ctrl-Z, Enter on Windows) or give a file with --input"
        );
    }
    let data = read_input(input).map_err(at(Stage::Io))?;
    let chomp = matches.get_flag("chomp") && *read_mode != ReadMode::Line;
    let mut data = data;
    let final_newline = match chomp {
        true => {
            let (body, final_newline) = split_final_newline(&data);
            let (length, final_newline) = (body.len(), final_newline.to_owned());
            data.truncate(length);
            final_newline
        }
        false => String::new(),
    };
    // The pipeline sees the input file as `__INPUT_FILE__` (empty for stdin).
    let input_file = input.map(PathBuf::as_path);
    let result = match matches.get_one::<PathBuf>("watch_report") {
        Some(report) => {
            if *read_mode != ReadMode::All {
                return Err(Failure {
                    stage: Stage::Usage,
                    error: TextForgeError::new(
                        TextForgeErrorCode::ValidationError(
                            "--watch reports one run of the pipeline, so it needs --read-mode all"
                                .into(),
                        ),
                        Cow::from("--watch"),
                        Cow::from(""),
                    ),
                });
            }
            let metrics: Vec<&str> = matches
                .get_many::<String>("watch")
                .into_iter()
                .flatten()
                .map(String::as_str)
                .collect();
            process_with_watchers(&mut processor, &id, &data, input_file, &metrics, report)
        }
        None => {
            let log = debug.then_some(&mut stdout as &mut dyn Write);
            process_input(&mut processor, &id, data, *read_mode, input_file, log)
        }
    };
    let mut result = match result {
        // The trace could not be written because its reader has gone away.
        Err(_) if stdout.closed => return Ok(()),
        result => result.map_err(at(Stage::Run))?,
    };
    result.push_str(&final_newline);

    match output {
        Some(path) => write_output(path, &result).map_err(at(Stage::Io))?,
        // With --debug the trace already went to stdout and ends with the
        // final text, so the result is not printed a second time after it.
        // Before 0.3.2 the result followed the trace on the same stream.
        None if debug => {}
        None => print_result(&mut stdout, &result).map_err(at(Stage::Io))?,
    }

    Ok(())
}

/// `txf compile` (from text) and `txf decompile` (from bytecode): load the
/// pipeline in `from` and write it in the other format.
fn convert(
    matches: &clap::ArgMatches,
    from: PipelineFormat,
    stdout: &mut Stdout,
) -> Result<(), Failure> {
    let source = argument::<PathBuf>(matches, "pipeline").map_err(at(Stage::Usage))?;
    let extension = match from {
        PipelineFormat::Text => "textforgebc",
        PipelineFormat::Bytecode => "textforge",
    };
    let target = matches
        .get_one::<PathBuf>("output")
        .cloned()
        .unwrap_or_else(|| source.with_extension(extension));
    let mut processor = TextForgeProcessor::new();
    let id = load_pipeline(&mut processor, source, Some(from)).map_err(at(Stage::Load))?;
    match from {
        PipelineFormat::Text => processor.write_to_bytecode_file(&id, &target),
        PipelineFormat::Bytecode => processor.write_pipeline_to_file(&id, &target),
    }
    .map_err(at(Stage::Io))?;
    print_result(
        stdout,
        &format!("{} -> {}\n", source.display(), target.display()),
    )
    .map_err(at(Stage::Io))
}

/// `txf check`: load every pipeline and report each on its own line.
fn check(matches: &clap::ArgMatches, stdout: &mut Stdout) -> Result<(), Failure> {
    let (mut total, mut failed) = (0, 0);
    for path in matches
        .get_many::<PathBuf>("pipelines")
        .into_iter()
        .flatten()
    {
        total += 1;
        let mut processor = TextForgeProcessor::new();
        let loaded = load_pipeline(&mut processor, path, None)
            .and_then(|id| processor.get_transform_vec(&id).map(|tokens| tokens.len()));
        match loaded {
            Ok(count) => {
                let plural = if count == 1 { "" } else { "s" };
                print_result(
                    stdout,
                    &format!("{}: ok, {count} instruction{plural}\n", path.display()),
                )
                .map_err(at(Stage::Io))?;
            }
            Err(error) => {
                failed += 1;
                eprintln!("{}: {error}", path.display());
            }
        }
    }
    if failed == 0 {
        return Ok(());
    }
    Err(Failure {
        stage: Stage::Load,
        error: TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("{failed} of {total} pipelines could not be loaded").into(),
            ),
            Cow::from("txf check"),
            Cow::from(""),
        ),
    })
}

/// The value of an argument that clap always fills (required or with a
/// default). A missing one is reported instead of panicking (Golden Rule 2).
fn argument<'a, T: Clone + Send + Sync + 'static>(
    matches: &'a clap::ArgMatches,
    name: &str,
) -> Result<&'a T, TextForgeError> {
    matches.get_one::<T>(name).ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::InvalidArgumentNumber(
                format!("internal error: the argument `{name}` has no value").into(),
            ),
            Cow::from("txf"),
            Cow::from(""),
        )
    })
}

/// `-`, the usual name of standard input or output.
fn is_dash(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// The text of the input file, or of standard input without one.
fn read_input(path: Option<&PathBuf>) -> Result<String, TextForgeError> {
    let Some(path) = path else {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text).map_err(|e| {
            TextForgeError::new(
                TextForgeErrorCode::FileReadingError(
                    format!("cannot read standard input as UTF-8 text: {e}").into(),
                ),
                Cow::from("txf"),
                Cow::from(""),
            )
        })?;
        return Ok(text);
    };
    if !path.exists() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::FileNotFound(
                format!("the input file `{}` does not exist", path.display()).into(),
            ),
            Cow::from("txf"),
            Cow::from(path.display().to_string()),
        ));
    }
    let mut file = OpenOptions::new().read(true).open(path).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::FileOpeningError(
                format!("cannot open the input file `{}`: {e}", path.display()).into(),
            ),
            Cow::from("txf"),
            Cow::from(path.display().to_string()),
        )
    })?;
    let mut text = String::new();
    file.read_to_string(&mut text).map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::FileReadingError(
                format!(
                    "cannot read the input file `{}` as UTF-8 text: {e}",
                    path.display()
                )
                .into(),
            ),
            Cow::from("txf"),
            Cow::from(path.display().to_string()),
        )
    })?;
    Ok(text)
}

/// Writes `text` to standard output; a reader that has gone away is not an
/// error.
fn print_result(stdout: &mut Stdout, text: &str) -> Result<(), TextForgeError> {
    match stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Err(_) if stdout.closed => Ok(()),
        Err(error) => Err(TextForgeError::new(
            TextForgeErrorCode::FileWritingError(
                format!("cannot write the result to standard output: {error}").into(),
            ),
            Cow::from("txf"),
            Cow::from(""),
        )),
        Ok(()) => Ok(()),
    }
}

#[cfg(feature = "test_access")]
#[cfg(test)]
mod textforge_tests {
    mod parser_tests {
        use crate::{ReadMode, build_cli};
        use std::{path::PathBuf, str::FromStr};

        #[test]
        fn config_show_needs_no_pipeline_and_accepts_profile_flags() {
            let parser = build_cli();
            let m = parser
                .try_get_matches_from(["txf", "config", "show", "--profile", "fast", "--no-rc"])
                .unwrap();
            let show = m
                .subcommand_matches("config")
                .and_then(|config| config.subcommand_matches("show"))
                .unwrap();
            let options = crate::profile_options(show);
            assert_eq!(options.profile.as_deref(), Some("fast"));
            assert!(options.no_rc);
            // --config and --no-rc contradict each other; runs still need --file.
            assert!(
                build_cli()
                    .try_get_matches_from(["txf", "-f", "p.textforge", "--config", "a", "--no-rc"])
                    .is_err()
            );
            assert!(
                build_cli()
                    .try_get_matches_from(["txf", "--profile", "fast"])
                    .is_err()
            );
            assert!(
                build_cli()
                    .try_get_matches_from(["txf", "-f", "p.textforge", "--profile", "turbo"])
                    .is_err()
            );
        }

        #[test]
        fn test_all_with_long_params() {
            let parser = build_cli();
            let c = "txf --file ./instructions.textforgebc --input ./example.txt --output output.txt --debug --mode b --read-mode line";

            let arg_vec: Vec<&str> = c.split_whitespace().collect();

            let m = parser.try_get_matches_from(arg_vec).unwrap();

            let file = m.get_one::<PathBuf>("file").unwrap();
            let input = m.get_one::<PathBuf>("input").unwrap();
            let output = m.get_one::<PathBuf>("output").unwrap();
            let textforge_mode = m.get_one::<String>("mode").unwrap();
            let read_mode = m.get_one::<ReadMode>("read_mode").unwrap();
            let debug = m.get_one::<bool>("debug").unwrap();

            assert_eq!(
                *file,
                PathBuf::from_str("./instructions.textforgebc").unwrap()
            );
            assert_eq!(*input, PathBuf::from_str("./example.txt").unwrap());
            assert_eq!(*output, PathBuf::from_str("output.txt").unwrap());
            assert_eq!(*textforge_mode, "b".to_string());
            assert_eq!(*read_mode, ReadMode::Line);
            assert!(*debug);
        }
        #[test]
        fn test_all_with_short_params() {
            let parser = build_cli();
            let c =
                "txf -f ./instructions.textforgebc -i ./example.txt -o output.txt -d -m b -r line";

            let arg_vec: Vec<&str> = c.split_whitespace().collect();

            let m = parser.try_get_matches_from(arg_vec).unwrap();

            let file = m.get_one::<PathBuf>("file").unwrap();
            let input = m.get_one::<PathBuf>("input").unwrap();
            let output = m.get_one::<PathBuf>("output").unwrap();
            let textforge_mode = m.get_one::<String>("mode").unwrap();
            let read_mode = m.get_one::<ReadMode>("read_mode").unwrap();
            let debug = m.get_one::<bool>("debug").unwrap();

            assert_eq!(
                *file,
                PathBuf::from_str("./instructions.textforgebc").unwrap()
            );
            assert_eq!(*input, PathBuf::from_str("./example.txt").unwrap());
            assert_eq!(*output, PathBuf::from_str("output.txt").unwrap());
            assert_eq!(*textforge_mode, "b".to_string());
            assert_eq!(*read_mode, ReadMode::Line);
            assert!(*debug);
        }
    }
}
