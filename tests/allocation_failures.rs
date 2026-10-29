//! Resource-limit regressions run in a child process, leaving the test runner unchanged.
#![cfg(all(feature = "test_access", target_os = "linux"))]

#[test]
fn repeat_reports_allocator_refusal_instead_of_aborting() {
    let directory = tempfile::tempdir().unwrap();
    let pipeline = directory.path().join("repeat.textforge");
    let input = directory.path().join("input.txt");
    let output = directory.path().join("output.txt");
    std::fs::write(&pipeline, "rpt 100;\n").unwrap();
    std::fs::write(&input, vec![b'x'; 8 * 1024 * 1024]).unwrap();

    // 800 MiB of output cannot fit in a 256 MiB address-space allowance.
    let result = std::process::Command::new("sh")
        .args([
            "-c",
            "ulimit -c 0 || exit 99; ulimit -v 262144 || exit 99; exec \"$@\"",
            "textforge-allocation-test",
        ])
        .arg(env!("CARGO_BIN_EXE_txf"))
        .arg("-f")
        .arg(&pipeline)
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();

    assert_eq!(
        result.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    // Native buffered repetition can spill the 800 MiB result successfully;
    // the String-returning API must then reject final materialization safely.
    let expected = if cfg!(feature = "buffered") {
        "Cannot materialize TextBuffer"
    } else {
        "Cannot allocate repeated output"
    };
    assert!(
        String::from_utf8_lossy(&result.stderr).contains(expected),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !output.exists(),
        "execution errors must not create partial output"
    );
}
