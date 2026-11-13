#[cfg(test)]
mod tests {
    use std::{borrow::Cow, path::Path};

    use crate::{
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::wtf::Wtf},
        utils::errors::TextForgeErrorCode,
    };

    fn token(path: &Path) -> Wtf {
        Wtf::new(&path.to_string_lossy())
    }

    fn leftovers(directory: &Path) -> Vec<String> {
        std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn writes_the_text_and_returns_it_without_copying() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("saída 📄.txt");
        for text in ["", "a", "é\u{301}👩🏽‍💻\r\nfim\n"] {
            let output = token(&path).transform(Cow::Borrowed(text), None).unwrap();
            assert!(matches!(output, Cow::Borrowed(borrowed) if std::ptr::eq(borrowed, text)));
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        }
        assert!(leftovers(directory.path()).is_empty());
    }

    #[test]
    fn replaces_an_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("out.txt");
        std::fs::write(&path, "a much longer previous content").unwrap();
        token(&path).transform("new".into(), None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
    }

    #[test]
    fn a_missing_directory_is_an_error_and_is_not_created() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("missing").join("out.txt");
        let error = token(&path).transform("x".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::FileNotFound(_)
        ));
        assert!(error.input.contains("missing"));
        assert!(!directory.path().join("missing").exists());
    }

    #[test]
    fn a_failed_write_keeps_the_previous_target_and_leaves_no_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        // The target is a non-empty directory: the final rename fails.
        let target = directory.path().join("taken");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("inside.txt"), "kept").unwrap();
        let error = token(&target).transform("x".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::FileWritingError(_)
        ));
        assert_eq!(
            std::fs::read_to_string(target.join("inside.txt")).unwrap(),
            "kept"
        );
        assert!(leftovers(directory.path()).is_empty());
    }

    #[test]
    fn a_path_without_file_name_is_an_error() {
        let error = Wtf::new("..").transform("x".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::FileWritingError(_)
        ));
    }

    #[test]
    fn params_and_text_line() {
        let mut token = Wtf::default();
        token
            .set_params(&[TextForgeParamTypes::String("out dir/a.txt".into())])
            .unwrap();
        assert_eq!(token.to_textforge_line(), "wtf out dir/a.txt;\n");
        assert_eq!(token.get_string_repr(), "wtf");
        assert!(token.set_params(&[]).is_err());
        assert!(token.set_params(&[TextForgeParamTypes::Usize(1)]).is_err());
    }

    #[cfg(feature = "bytecode")]
    #[test]
    fn opcode() {
        assert_eq!(Wtf::default().get_opcode(), 0x55);
    }

    #[cfg(feature = "buffered")]
    #[test]
    fn spooled_documents_are_streamed_to_the_file() {
        use crate::api::{TextBuffer, TextBufferOptions};
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("big.txt");
        let text = "linha é 🍌\n".repeat(20_000);
        let buffer =
            TextBuffer::from_text(&text, TextBufferOptions::new(64, directory.path())).unwrap();
        assert!(buffer.is_spilled());
        let output = token(&path).transform_buffered(buffer, None).unwrap();
        assert!(output.is_spilled());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
        assert_eq!(output.into_string().unwrap(), text);
    }
}
