use std::{fs::OpenOptions, io::Write, path::Path};

use crate::{
    parser::resolve_var::TokenWrapper,
    utils::{errors::TextForgeError, validations::check_file_path},
};

pub fn write_to_file(path: &Path, tokens: &[TokenWrapper]) -> Result<(), TextForgeError> {
    let lines = tokens
        .iter()
        .map(TokenWrapper::to_text_line_unresolved)
        .collect::<Result<Vec<_>, _>>()?;
    check_file_path(path, Some("textforge"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .map_err(|_| {
            TextForgeError::new(
                crate::utils::errors::TextForgeErrorCode::FileOpeningError(
                    "Failed opening File".into(),
                ),
                "",
                format!("{:?}", path),
            )
        })?;

    let mut success = true;

    for line in lines {
        match file.write_all(line.as_bytes()) {
            Ok(_) => (),
            Err(_) => {
                success = false;
            }
        }
    }

    match success {
        true => Ok(()),
        false => Err(TextForgeError::new(
            crate::utils::errors::TextForgeErrorCode::FileWritingError(
                "Failed writing text to textforge file".into(),
            ),
            "",
            "",
        )),
    }
}
