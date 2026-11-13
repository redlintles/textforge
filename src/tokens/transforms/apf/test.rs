#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::Path};

    use crate::{
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::apf::Apf},
        utils::errors::TextForgeErrorCode,
    };

    fn token(path: &Path) -> Apf {
        Apf::new(&path.to_string_lossy())
    }

    #[test]
    fn appends_the_file_to_the_end() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tail.txt");
        std::fs::write(&path, "\r\nfim e\u{301} 👩🏽‍💻").unwrap();
        for input in ["", "início", "🍌"] {
            assert_eq!(
                token(&path).transform(input.into(), None).unwrap(),
                format!("{input}\r\nfim e\u{301} 👩🏽‍💻")
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
        let mut token = Apf::default();
        token
            .set_params(&[TextForgeParamTypes::String("a.txt".into())])
            .unwrap();
        assert_eq!(token.to_textforge_line(), "apf a.txt;\n");
        assert_eq!(token.get_string_repr(), "apf");
        assert!(token.set_params(&[TextForgeParamTypes::Usize(1)]).is_err());
    }

    #[cfg(feature = "bytecode")]
    #[test]
    fn opcode() {
        assert_eq!(Apf::default().get_opcode(), 0x56);
    }

    #[cfg(feature = "buffered")]
    #[test]
    fn spooled_documents_stay_spooled() {
        use crate::api::{TextBuffer, TextBufferOptions};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tail.txt");
        std::fs::write(&path, "🍌 fim").unwrap();
        let text = "é\n".repeat(10_000);
        let buffer =
            TextBuffer::from_text(&text, TextBufferOptions::new(64, directory.path())).unwrap();
        let output = token(&path).transform_buffered(buffer, None).unwrap();
        assert!(output.is_spilled());
        assert_eq!(output.into_string().unwrap(), format!("{text}🍌 fim"));
    }
}
