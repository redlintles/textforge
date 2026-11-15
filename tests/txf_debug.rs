//! `txf --debug` prints only the trace.
#![cfg(all(feature = "cli", feature = "test_access"))]

use std::io::Write;
use std::process::{Command, Stdio};

fn txf(pipeline: &str, input: &str, extra: &[&str]) -> (String, tempfile::TempDir) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("p.textforge");
    std::fs::write(&path, pipeline).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_txf"))
        .arg("-f")
        .arg(&path)
        .args(extra)
        .current_dir(directory.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    (String::from_utf8(output.stdout).unwrap(), directory)
}

/// Before: the result was printed again after the trace, glued to its last
/// line, so the output could not be read as a trace.
#[test]
fn debug_prints_the_trace_without_the_result_after_it() {
    let (plain, _) = txf("ate RESULT;\n", "abc", &[]);
    assert_eq!(plain, "abcRESULT");

    let (traced, _) = txf("ate RESULT;\n", "abc", &["-d"]);
    assert!(traced.starts_with("PROCESSING STEP BY STEP:"), "{traced}");
    assert!(traced.contains("After: abcRESULT"), "{traced}");
    assert_eq!(traced.matches("abcRESULT").count(), 1, "{traced}");
}

/// With `--output`, the trace goes to stdout and the result to the file.
#[test]
fn debug_with_output_still_writes_the_result() {
    let (traced, directory) = txf("ate !;\n", "abc", &["-d", "-o", "out.txt"]);
    assert!(traced.starts_with("PROCESSING STEP BY STEP:"), "{traced}");
    assert_eq!(
        std::fs::read_to_string(directory.path().join("out.txt")).unwrap(),
        "abc!"
    );
}
