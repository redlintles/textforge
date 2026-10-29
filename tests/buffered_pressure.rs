//! Exercise a result larger than the child process address-space limit.
#![cfg(all(feature = "buffered", target_os = "linux"))]

#[test]
fn native_repetition_does_not_materialize_its_output() {
    const CHILD: &str = "TEXTFORGE_BUFFERED_MEMORY_CHILD";
    if std::env::var_os(CHILD).is_some() {
        use std::io::Read;
        use textforge::api::processor::TextForgeProcessor;
        use textforge::api::{TextBuffer, TextBufferOptions, TextForgeBuilderMethods};
        let directory = tempfile::tempdir().unwrap();
        let options = TextBufferOptions::new(1024 * 1024, directory.path());
        let input =
            TextBuffer::from_reader(std::io::repeat(b'x').take(4 * 1024 * 1024), options).unwrap();
        let mut processor = TextForgeProcessor::new();
        let id = processor
            .create_pipeline()
            .repeat(32)
            .unwrap()
            .build()
            .unwrap();
        let mut result = processor.process_all_buffered(&id, input).unwrap();
        assert!(result.is_spilled());
        assert_eq!(result.len_bytes(), 128 * 1024 * 1024);
        assert_eq!(result.copy_to(std::io::sink()).unwrap(), 128 * 1024 * 1024);
        return;
    }
    let result = std::process::Command::new("sh")
        .args([
            "-c",
            "ulimit -c 0 || exit 99; ulimit -v 98304 || exit 99; exec \"$@\"",
            "textforge-buffered-pressure",
        ])
        .arg(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_repetition_does_not_materialize_its_output",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
