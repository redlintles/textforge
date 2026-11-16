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
