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
        cli::{ProfileOptions, ReadMode, process_input, resolve_profile, split_final_newline},
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_file_path,
    },
};

/// Flags choosing the execution profile, shared by runs and `config show`.
fn profile_args() -> [Arg; 6] {
    [
        Arg::new("max_generated_bytes")
            .long("max-generated-bytes")
            .value_name("SIZE|unlimited")
            .global(true)
            .help("Largest text rpt/padl/padr may produce (default 2GiB)"),
        Arg::new("max_iterations")
            .long("max-iterations")
            .value_name("N|unlimited")
            .global(true)
            .help("Largest iter count (default 10000000)"),
        Arg::new("profile")
            .long("profile")
            .value_name("PRESET")
            .value_parser(["balanced", "low-memory", "fast"])
            .global(true)
            .help("Execution profile preset (memory/speed trade-off; never changes results)"),
        Arg::new("spool_threshold")
            .long("spool-threshold")
            .value_name("SIZE")
            .global(true)
            .help("Override spool_threshold: bytes or e.g. 512MiB (buffered builds only)"),
        Arg::new("config")
            .long("config")
            .value_name("PATH")
            .value_parser(value_parser!(PathBuf))
            .conflicts_with("no_rc")
            .global(true)
            .help("Read this configuration file instead of ./.txfrc"),
        Arg::new("no_rc")
            .long("no-rc")
            .action(ArgAction::SetTrue)
            .global(true)
            .help("Do not read ./.txfrc"),
    ]
}

fn profile_options(matches: &clap::ArgMatches) -> ProfileOptions {
    ProfileOptions {
        profile: matches.get_one::<String>("profile").cloned(),
        spool_threshold: matches.get_one::<String>("spool_threshold").cloned(),
        max_generated_bytes: matches.get_one::<String>("max_generated_bytes").cloned(),
        max_iterations: matches.get_one::<String>("max_iterations").cloned(),
        config: matches.get_one::<PathBuf>("config").cloned(),
        no_rc: matches.get_flag("no_rc"),
    }
}

fn build_cli() -> Command {
    Command::new("txf")
        .version(env!("CARGO_PKG_VERSION"))
        .about("CLI for TextForge")
        .subcommand_negates_reqs(true)
        .args(profile_args())
        .subcommand(
            Command::new("config")
                .about("Inspect the engine configuration")
                .subcommand_required(true)
                .subcommand(
                    Command::new("show")
                        .about("Print the effective profile and where each value comes from"),
                ),
        )
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .required(true)
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .help("The pipeline file: a .textforge text pipeline, or a .textforgebc bytecode pipeline with --mode b")
        )
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .required(false)
                .value_name("INPUT")
                .value_parser(value_parser!(PathBuf))
                .help(
                    "The file to process; without it, or with -, txf reads standard input"
                )
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .required(false)
                .value_name("OUTPUT")
                .value_parser(value_parser!(PathBuf))
                .help(
                    "The file to write the result to; without it, or with -, txf prints the result"
                )
        )
        .arg(
            Arg::new("read_mode")
                .short('r')
                .long("read-mode")
                .default_value("all")
                .required(false)
                .value_name("READ_MODE")
                .value_parser(|s: &str| s.parse::<ReadMode>())
                .help(
                    "How to feed the input to the pipeline: 'all' (default) processes it as one text, 'line' processes each line separately, and 'chunk-N' processes it in pieces of N characters"
                )
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .required(false)
                .value_name("mode")
                .value_parser(["b", "t"])
                .default_value("t")
                .help(
                    "How to read --file: 't' for a text pipeline (default) or 'b' for a bytecode pipeline"
                )
        )
        .arg(
            Arg::new("chomp")
                .long("chomp")
                .required(false)
                .action(ArgAction::SetTrue)
                .help(
                    "Set the final line break of the input aside and put it back on the result, so instructions such as dll and ate work on the last visible character (--read-mode line always does this for every line)"
                )
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .required(false)
                .value_name("debug")
                .action(ArgAction::SetTrue)
                .help(
                    "Print only the step-by-step trace of the run instead of the result; with --output the result is still written to the file (default: off)"
                )
        )
}

/// Loads the pipeline in `file` in the requested mode and returns its id and the
/// processor. Mode `b` (bytecode) needs the `bytecode` feature; without it,
/// choosing `b` fails at run time with a clear message instead of the program
/// failing to build.
fn load_pipeline(mode: &str, file: &Path) -> Result<(String, TextForgeProcessor), TextForgeError> {
    match mode {
        "b" => {
            #[cfg(feature = "bytecode")]
            {
                check_file_path(file, Some("textforgebc"))?;
                let mut processor = TextForgeProcessor::new();
                let id = processor.read_from_bytecode_file(file)?;
                Ok((id, processor))
            }
            #[cfg(not(feature = "bytecode"))]
            {
                Err(TextForgeError::new(
                    TextForgeErrorCode::ValidationError(
                        "bytecode mode (`--mode b`) needs a txf built with `--features bytecode`"
                            .into(),
                    ),
                    Cow::from("txf"),
                    Cow::from(""),
                ))
            }
        }
        "t" => {
            check_file_path(file, Some("textforge"))?;
            let mut processor = TextForgeProcessor::new();
            let id = processor.get_pipeline_from_file(file)?;
            Ok((id, processor))
        }
        // clap's value_parser restricts "mode" to "b" and "t".
        _ => unreachable!("clap value_parser restricts mode to b/t"),
    }
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

/// Runs the program and prints an error the way a person reads it
/// (`error[12]: message`, then the instruction and the input) on standard
/// error. Before 0.3.2 the error was printed as a Rust debug structure.
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), TextForgeError> {
    let matches = build_cli().get_matches();
    let profile = resolve_profile(&profile_options(&matches), Path::new("."))?;
    let mut stdout = Stdout::new();
    if matches.subcommand_matches("config").is_some() {
        return print_result(&mut stdout, &profile.describe());
    }

    let file = matches.get_one::<PathBuf>("file").unwrap();
    let input = matches.get_one::<PathBuf>("input");
    let output = matches.get_one::<PathBuf>("output");
    let textforge_mode = matches.get_one::<String>("mode").unwrap();
    let read_mode = matches.get_one::<ReadMode>("read_mode").unwrap();
    let debug = *matches.get_one::<bool>("debug").unwrap();

    // `-` is standard input for --input and standard output for --output.
    let input = input.filter(|path| !is_dash(path));
    let output = output.filter(|path| !is_dash(path));

    // Load the pipeline first, so a mistake in it is reported at once instead
    // of after the whole input has been read. Before 0.3.2 the input came first.
    let (id, mut processor) = load_pipeline(textforge_mode.as_str(), file)?;
    processor.set_profile(profile.profile);
    if input.is_none() && io::stdin().is_terminal() {
        eprintln!(
            "txf: reading the text from standard input; end it with Ctrl-D (Ctrl-Z, Enter on Windows) or give a file with --input"
        );
    }
    let data = read_input(input)?;
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
    let log = debug.then_some(&mut stdout as &mut dyn Write);
    let result = process_input(
        &mut processor,
        &id,
        data,
        *read_mode,
        input.map(PathBuf::as_path),
        log,
    );
    let mut result = match result {
        // The trace could not be written because its reader has gone away.
        Err(_) if stdout.closed => return Ok(()),
        result => result?,
    };
    result.push_str(&final_newline);

    match output {
        Some(p) => {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(p)
                .map_err(|e| {
                    TextForgeError::new(
                        TextForgeErrorCode::FileOpeningError(
                            format!("cannot create the output file `{}`: {e}", p.display()).into(),
                        ),
                        Cow::from("txf"),
                        Cow::from(p.display().to_string()),
                    )
                })?;

            f.write_all(result.as_bytes()).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileWritingError(
                        format!("cannot write the output file `{}`: {e}", p.display()).into(),
                    ),
                    Cow::from("txf"),
                    Cow::from(p.display().to_string()),
                )
            })?;
        }
        // With --debug the trace already went to stdout and ends with the
        // final text, so the result is not printed a second time after it.
        // Before 0.3.2 the result followed the trace on the same stream.
        None if debug => {}
        None => print_result(&mut stdout, &result)?,
    }

    Ok(())
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
            assert!(m.subcommand_matches("config").is_some());
            let options = crate::profile_options(&m);
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
