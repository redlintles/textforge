use super::*;

fn options(limit: usize) -> TextBufferOptions {
    TextBufferOptions::new(limit, std::env::temp_dir())
}

#[test]
fn threshold_and_consuming_materialization() {
    let small = TextBuffer::from_text("abcd", options(4)).unwrap();
    assert!(!small.is_spilled());
    assert_eq!(small.len_bytes(), 4);
    assert_eq!(small.into_string().unwrap(), "abcd");
    let large = TextBuffer::from_text("abcde", options(4)).unwrap();
    assert!(large.is_spilled());
    assert_eq!(large.into_string().unwrap(), "abcde");
}

#[test]
fn zero_threshold_empty_and_explicit_spill() {
    let mut empty = TextBuffer::from_text("", options(0)).unwrap();
    assert!(empty.is_empty());
    empty.spill().unwrap();
    assert!(empty.is_spilled());
    assert_eq!(empty.into_string().unwrap(), "");
}

#[test]
fn unicode_code_points_survive_one_byte_reads() {
    struct OneByte<'a>(&'a [u8]);
    impl Read for OneByte<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let len = output.len().min(1);
            self.0.read(&mut output[..len])
        }
    }
    let input = "é 👩🏽‍💻 🇧🇷\0";
    let buffer = TextBuffer::from_reader(OneByte(input.as_bytes()), options(1)).unwrap();
    assert_eq!(buffer.into_string().unwrap(), input);
}

#[test]
fn rejects_invalid_and_incomplete_utf8() {
    for bytes in [&[0xff][..], &[0xf0, 0x9f][..], &[b'a', 0xc3, b'b'][..]] {
        assert!(matches!(
            TextBuffer::from_reader(bytes, options(1))
                .unwrap_err()
                .error_code,
            TextForgeErrorCode::InvalidUtf8Error(_)
        ));
    }
}

#[test]
fn independent_reads_and_copies_ignore_previous_cursor() {
    let mut buffer = TextBuffer::from_text("abc👩🏽‍💻", options(1)).unwrap();
    let mut part = [0; 2];
    assert_eq!(buffer.read_at(1, &mut part).unwrap(), 2);
    assert_eq!(&part, b"bc");
    for _ in 0..2 {
        let mut result = Vec::new();
        buffer.copy_to(&mut result).unwrap();
        assert_eq!(result, "abc👩🏽‍💻".as_bytes());
    }
    assert_eq!(buffer.into_string().unwrap(), "abc👩🏽‍💻");
}

#[test]
fn reports_io_errors_without_exposing_tempfile() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing");
    assert!(TextBuffer::from_text("text", TextBufferOptions::new(0, missing)).is_err());
    struct BadWriter;
    impl Write for BadWriter {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut buffer = TextBuffer::from_text("text", options(1)).unwrap();
    assert!(matches!(
        buffer.copy_to(BadWriter).unwrap_err().error_code,
        TextForgeErrorCode::FileWritingError(_)
    ));
    assert_eq!(buffer.into_string().unwrap(), "text");
}

#[test]
fn bounded_operations_match_strings_in_both_storage_modes() {
    for threshold in [0, 1, 1024 * 1024] {
        for text in ["", "a", "é", "👩🏽‍💻", "🇧🇷\n"] {
            for count in [0, 1, 2, 3, 17, 65537] {
                let mut buffer = TextBuffer::from_text(text, options(threshold))
                    .unwrap()
                    .repeat(count)
                    .unwrap()
                    .prepend("<")
                    .unwrap();
                buffer.append_text(">").unwrap();
                assert_eq!(
                    buffer.into_string().unwrap(),
                    format!("<{}>", text.repeat(count))
                );
            }
        }
    }
}

#[test]
fn reader_failure_and_repeat_overflow_return_errors() {
    struct BadReader;
    impl Read for BadReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("test failure"))
        }
    }
    assert!(TextBuffer::from_reader(BadReader, options(1)).is_err());
    assert!(
        TextBuffer::from_text("ab", options(1))
            .unwrap()
            .repeat(usize::MAX)
            .is_err()
    );
}
