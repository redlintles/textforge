//! The `txf` program run as a process: one test per fixed behavior, each
//! naming what happened before the fix (docs/audits/0.3.2-cli-audit.md).
#![cfg(all(feature = "cli", feature = "test_access"))]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// A directory holding the files of one test.
struct Files(tempfile::TempDir);

impl Files {
    fn new() -> Self {
        Files(tempfile::tempdir().unwrap())
    }
    fn path(&self) -> &Path {
        self.0.path()
    }
    fn write(&self, name: &str, contents: impl AsRef<[u8]>) -> &Self {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
        self
    }
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.path().join(name)).unwrap()
    }
    /// Runs `txf` in this directory with `stdin` as standard input.
    fn txf(&self, args: &[&str], stdin: &str) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_txf"))
            .args(args)
            .arg("--no-rc")
            .current_dir(self.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        // The program may exit before reading (for example on a usage error).
        let _ = child.stdin.take().unwrap().write_all(stdin.as_bytes());
        child.wait_with_output().unwrap()
    }
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "status {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

/// Before: `txf` ran the pipeline without the input file, so
/// `__INPUT_FILE__` was always empty.
#[test]
fn the_input_file_reaches_the_pipeline() {
    let files = Files::new();
    files
        .write("p.textforge", "atb \"{{__INPUT_FILE__}}|\";\n")
        .write("in.txt", "abc");
    let output = files.txf(&["-f", "p.textforge", "-i", "in.txt"], "");
    assert_eq!(stdout(&output), "in.txt|abc");
    // In every read mode and with the trace.
    let output = files.txf(&["-f", "p.textforge", "-i", "in.txt", "-r", "line"], "");
    assert_eq!(stdout(&output), "in.txt|abc");
    let output = files.txf(&["-f", "p.textforge", "-i", "in.txt", "-d"], "");
    assert!(stdout(&output).contains("After: in.txt|abc"));
    // Standard input has no file.
    let output = files.txf(&["-f", "p.textforge"], "abc");
    assert_eq!(stdout(&output), "|abc");
}

/// Before: writing to a closed pipe panicked ("failed printing to stdout:
/// Broken pipe") and the program exited with 101.
#[test]
fn a_closed_pipe_ends_the_program_quietly() {
    let files = Files::new();
    files.write("p.textforge", "rpt 100000;\n");
    for extra in [&[][..], &["-d"][..]] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_txf"))
            .args(["-f", "p.textforge", "--no-rc"])
            .args(extra)
            .current_dir(files.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"0123456789\n")
            .unwrap();
        // Read a little, then close the pipe before the program has written everything.
        let mut out = child.stdout.take().unwrap();
        let mut start = [0u8; 4];
        std::io::Read::read_exact(&mut out, &mut start).unwrap();
        drop(out);
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!stderr.contains("panicked"), "{extra:?}: {stderr}");
        assert_eq!(output.status.code(), Some(0), "{extra:?}: {stderr}");
    }
}

/// Before: `-i -` failed with "the input file `-` does not exist" and `-o -`
/// wrote a file called `-`.
#[test]
fn a_dash_is_standard_input_and_output() {
    let files = Files::new();
    files.write("p.textforge", "tua;\n");
    let output = files.txf(&["-f", "p.textforge", "-i", "-", "-o", "-"], "abc");
    assert_eq!(stdout(&output), "ABC");
    assert!(!files.path().join("-").exists());
}

/// Before: the whole input was read before the pipeline was loaded, so a
/// mistake in the pipeline was reported only when standard input closed.
#[test]
fn a_broken_pipeline_is_reported_before_reading_the_input() {
    let files = Files::new();
    files.write("p.textforge", "nope;\n");
    let mut child = Command::new(env!("CARGO_BIN_EXE_txf"))
        .args(["-f", "p.textforge", "--no-rc"])
        .current_dir(files.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // Standard input stays open and empty while the program runs.
    let _stdin = child.stdin.take().unwrap();
    let started = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > std::time::Duration::from_secs(10) {
            child.kill().unwrap();
            panic!("txf waited for standard input before loading the pipeline");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert!(!status.success());
    let output = child.wait_with_output().unwrap();
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown instruction `nope`"));
}

/// Before: `--read-mode line` and `--chomp` knew only LF and CRLF, unlike the
/// line rules of `docs/lines.md` that `__LINES__` and the line instructions use.
#[test]
fn line_mode_and_chomp_use_the_line_rules_of_the_engine() {
    let files = Files::new();
    files.write("p.textforge", "ate !;\n");
    let output = files.txf(&["-f", "p.textforge", "-r", "line"], "a\rb\u{2028}c");
    assert_eq!(stdout(&output), "a!\rb!\u{2028}c!");
    let output = files.txf(&["-f", "p.textforge", "--chomp"], "abc\r");
    assert_eq!(stdout(&output), "abc!\r");
    let output = files.txf(&["-f", "p.textforge", "--chomp"], "abc\r\n");
    assert_eq!(stdout(&output), "abc!\r\n");
}

/// Before: a chunk could end between `\r` and `\n`, and an error in a chunk did
/// not say which one failed (line mode says `input line N ›`).
#[test]
fn chunks_keep_crlf_whole_and_errors_name_the_chunk() {
    let files = Files::new();
    files.write("p.textforge", "atb X;\n");
    let output = files.txf(&["-f", "p.textforge", "-r", "chunk-3"], "ab\r\ncd");
    assert_eq!(stdout(&output), "Xab\r\nXcd");
    files.write("q.textforge", "dlc 2 2;\n");
    let output = files.txf(&["-f", "q.textforge", "-r", "chunk-3"], "abcdefg");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("input chunk 3 › dlc 2 2;"), "{stderr}");
}

/// Before: the traces of `line` and `chunk` mode followed one another without
/// saying which piece they belonged to, and a pipeline without instructions
/// printed only the heading of the trace.
#[test]
fn traces_name_their_piece_and_an_empty_pipeline_says_so() {
    let files = Files::new();
    files.write("p.textforge", "tua;\n");
    let trace = stdout(&files.txf(&["-f", "p.textforge", "-d", "-r", "line"], "a\nb\n"));
    let first = trace.find("=== input line 1 ===").unwrap();
    let second = trace.find("=== input line 2 ===").unwrap();
    assert!(first < trace.find("After: A").unwrap());
    assert!(second < trace.find("After: B").unwrap() && first < second);
    let trace = stdout(&files.txf(&["-f", "p.textforge", "-d", "-r", "chunk-2"], "abcd"));
    assert!(trace.contains("=== input chunk 2 ==="), "{trace}");
    files.write("empty.textforge", "# nothing yet\n");
    let trace = stdout(&files.txf(&["-f", "empty.textforge", "-d"], "abc"));
    assert!(trace.contains("no instructions"), "{trace}");
}

/// Before: a bytecode pipeline needed `--mode b`; without it, txf tried to
/// read the bytecode as text.
#[test]
fn the_pipeline_format_is_detected_from_the_file() {
    let files = Files::new();
    files.write("p.textforge", "tua;\n");
    let mut processor = textforge::api::processor::TextForgeProcessor::new();
    let id = textforge::utils::cli::load_pipeline(
        &mut processor,
        &files.path().join("p.textforge"),
        None,
    )
    .unwrap();
    textforge::api::processor::TextForgeProcessorMethods::write_to_bytecode_file(
        &mut processor,
        &id,
        &files.path().join("p.tfb"),
    )
    .unwrap();
    for args in [
        &["-f", "p.tfb"][..],
        &["-f", "p.tfb", "-m", "b"][..],
        &["-f", "p.tfb", "--mode", "bytecode"][..],
        &["-f", "p.textforge"][..],
        &["-f", "p.textforge", "-m", "t"][..],
    ] {
        assert_eq!(stdout(&files.txf(args, "abc")), "ABC", "{args:?}");
    }
    // A forced format still checks the extension.
    let output = files.txf(&["-f", "p.tfb", "-m", "t"], "abc");
    assert!(!output.status.success());
}

/// Before: every failure exited with 1, so a script could not tell a pipeline
/// that rejected its text from a typo in the pipeline or a missing file.
#[test]
fn the_exit_status_says_which_step_failed() {
    let files = Files::new();
    files
        .write("ok.textforge", "tua;\n")
        .write("bad.textforge", "nope;\n")
        .write("dlc.textforge", "dlc 9 9;\n");
    let status = |args: &[&str]| files.txf(args, "abc").status.code();
    assert_eq!(status(&["-f", "ok.textforge"]), Some(0));
    assert_eq!(status(&["-f", "dlc.textforge"]), Some(1));
    assert_eq!(status(&["-f", "ok.textforge", "--bogus"]), Some(2));
    assert_eq!(
        status(&["-f", "ok.textforge", "--max-iterations", "x"]),
        Some(2)
    );
    assert_eq!(status(&["-f", "bad.textforge"]), Some(3));
    assert_eq!(status(&["-f", "missing.textforge"]), Some(3));
    assert_eq!(
        status(&["-f", "ok.textforge", "-i", "missing.txt"]),
        Some(4)
    );
    assert_eq!(
        status(&["-f", "ok.textforge", "-o", "no-such-dir/out.txt"]),
        Some(4)
    );
}

/// Before: there was no way to compile, decompile or check a pipeline from
/// the command line.
#[test]
fn pipelines_can_be_compiled_decompiled_and_checked() {
    let files = Files::new();
    files
        .write("lib.textforge", "tua;\n")
        .write("p.textforge", "ppjn \"lib.textforge\";\nate !;\n")
        .write("bad.textforge", "nope;\n");
    let output = files.txf(&["compile", "p.textforge"], "");
    assert_eq!(stdout(&output), "p.textforge -> p.textforgebc\n");
    // The joined pipeline is embedded: the bytecode runs without its file.
    std::fs::remove_file(files.path().join("lib.textforge")).unwrap();
    assert_eq!(stdout(&files.txf(&["-f", "p.textforgebc"], "ab")), "AB!");
    let output = files.txf(&["decompile", "p.textforgebc", "-o", "back.tft"], "");
    assert_eq!(stdout(&output), "p.textforgebc -> back.tft\n");
    // As text, a joined pipeline is a `ppjn` again, so its file is needed.
    let text = files.read("back.tft");
    assert!(
        text.contains("ppjn") && text.contains("lib.textforge") && text.contains("ate !;"),
        "{text}"
    );
    files.write("lib.textforge", "tua;\n");
    // compile reads text, decompile reads bytecode.
    assert_eq!(
        files.txf(&["compile", "p.textforgebc"], "").status.code(),
        Some(3)
    );
    let output = files.txf(&["check", "back.tft", "p.textforgebc", "bad.textforge"], "");
    assert_eq!(output.status.code(), Some(3));
    let report = String::from_utf8_lossy(&output.stdout);
    assert!(report.contains("back.tft: ok, 2 instructions"), "{report}");
    assert!(
        report.contains("p.textforgebc: ok, 2 instructions"),
        "{report}"
    );
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("bad.textforge: "), "{errors}");
    assert!(
        errors.contains("1 of 3 pipelines could not be loaded"),
        "{errors}"
    );
}
