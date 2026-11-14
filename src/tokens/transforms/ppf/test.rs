#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::Path};

    use crate::{
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::ppf::Ppf},
        utils::errors::TextForgeErrorCode,
    };

    fn token(path: &Path) -> Ppf {
        Ppf::new(&path.to_string_lossy())
    }

    #[test]
    fn prepends_the_file_to_the_beginning() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("head.txt");
        std::fs::write(&path, "início e\u{301} 👩🏽‍💻\r\n").unwrap();
        for input in ["", "fim", "🍌"] {
            assert_eq!(
                token(&path).transform(input.into(), None).unwrap(),
                format!("início e\u{301} 👩🏽‍💻\r\n{input}")
            );
        }
    }

    #[test]
    fn an_empty_file_returns_the_input_without_copying() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("empty.txt");
        std::fs::write(&path, "").unwrap();
        let input = "abc";
        let output = token(&path).transform(Cow::Borrowed(input), None).unwrap();
        assert!(matches!(output, Cow::Borrowed(text) if std::ptr::eq(text, input)));
    }

    #[test]
    fn missing_files_and_invalid_utf8_are_errors() {
        let directory = tempfile::tempdir().unwrap();
        let missing = token(&directory.path().join("missing.txt"))
            .transform("x".into(), None)
            .unwrap_err();
        assert!(matches!(
            missing.error_code,
            TextForgeErrorCode::FileOpeningError(_)
        ));
        assert!(missing.input.contains("missing.txt"));

        let invalid = directory.path().join("invalid.txt");
        std::fs::write(&invalid, [b'a', 0xFF, b'b']).unwrap();
        let error = token(&invalid).transform("x".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::InvalidUtf8Error(_)
        ));
    }

    #[test]
    fn params_and_text_line() {
        let mut token = Ppf::default();
        token
            .set_params(&[TextForgeParamTypes::String("a.txt".into())])
            .unwrap();
        assert_eq!(token.to_textforge_line(), "ppf a.txt;\n");
        assert_eq!(token.get_string_repr(), "ppf");
        assert!(token.set_params(&[TextForgeParamTypes::Usize(1)]).is_err());
        assert!(token.set_params(&[]).is_err());
    }

    #[cfg(feature = "bytecode")]
    #[test]
    fn opcode_and_bytecode_round_trip() {
        assert_eq!(Ppf::default().get_opcode(), 0x59);
        let wrapper = crate::text::reader::read_from_text("ppf 'dir/a b.txt';").unwrap();
        let mut bytes = vec![38, 235, 245, 8, 244, 137, 1, 179];
        bytes.extend_from_slice(&1u64.to_be_bytes());
        bytes.extend_from_slice(&1u32.to_be_bytes());
        bytes.extend(wrapper.to_bytecode_unresolved().unwrap());
        let read = crate::bytecode::reader::read_bytecode_from_bytes(&bytes).unwrap();
        assert_eq!(
            read[0].to_text_line_unresolved().unwrap(),
            "ppf 'dir/a b.txt';\n"
        );
    }

    #[cfg(feature = "buffered")]
    #[test]
    fn spooled_documents_stay_spooled() {
        use crate::api::{TextBuffer, TextBufferOptions};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("head.txt");
        std::fs::write(&path, "início 🍌\n").unwrap();
        let text = "é\n".repeat(10_000);
        let buffer =
            TextBuffer::from_text(&text, TextBufferOptions::new(64, directory.path())).unwrap();
        let output = token(&path).transform_buffered(buffer, None).unwrap();
        assert!(output.is_spilled());
        assert_eq!(output.into_string().unwrap(), format!("início 🍌\n{text}"));
    }
}
