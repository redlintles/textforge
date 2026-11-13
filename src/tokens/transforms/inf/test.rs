#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        parser::params::TextForgeParamTypes,
        tokens::{
            InstructionMethods,
            transforms::{inf::Inf, ins::Ins},
        },
        utils::errors::TextForgeErrorCode,
    };

    fn token(path: &Path, index: usize) -> Inf {
        Inf::new(&path.to_string_lossy(), index)
    }

    #[test]
    fn follows_the_index_rules_of_ins_exactly() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("piece.txt");
        let piece = "<é👩🏽‍💻>";
        std::fs::write(&path, piece).unwrap();
        for input in ["", "a", "abc", "e\u{301}🍌x", "\r\n"] {
            for index in 0..=input.chars().count() + 1 {
                let expected = Ins::new(index, piece)
                    .transform(input.into(), None)
                    .map(|text| text.into_owned())
                    .map_err(|error| error.error_code);
                let actual = token(&path, index)
                    .transform(input.into(), None)
                    .map(|text| text.into_owned())
                    .map_err(|error| error.error_code);
                assert_eq!(
                    format!("{actual:?}"),
                    format!("{expected:?}"),
                    "input {input:?}, index {index}"
                );
            }
        }
    }

    #[test]
    fn an_out_of_range_index_is_an_index_error() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("piece.txt");
        std::fs::write(&path, "x").unwrap();
        let error = token(&path, 5).transform("ab".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::IndexOutOfRange(_)
        ));
    }

    #[test]
    fn missing_files_and_invalid_utf8_are_errors() {
        let directory = tempfile::tempdir().unwrap();
        let missing = token(&directory.path().join("missing.txt"), 0)
            .transform("x".into(), None)
            .unwrap_err();
        assert!(matches!(
            missing.error_code,
            TextForgeErrorCode::FileOpeningError(_)
        ));

        let invalid = directory.path().join("invalid.txt");
        std::fs::write(&invalid, [0xC3, 0x28]).unwrap();
        let error = token(&invalid, 0).transform("x".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::InvalidUtf8Error(_)
        ));
    }

    #[test]
    fn params_and_text_line() {
        let mut token = Inf::default();
        token
            .set_params(&[
                TextForgeParamTypes::String("a.txt".into()),
                TextForgeParamTypes::Usize(3),
            ])
            .unwrap();
        assert_eq!(token.to_textforge_line(), "inf a.txt 3;\n");
        assert_eq!(token.get_string_repr(), "inf");
        assert!(
            token
                .set_params(&[
                    TextForgeParamTypes::Usize(3),
                    TextForgeParamTypes::String("a.txt".into())
                ])
                .is_err()
        );
    }

    #[cfg(feature = "bytecode")]
    #[test]
    fn opcode() {
        assert_eq!(Inf::default().get_opcode(), 0x57);
    }
}
