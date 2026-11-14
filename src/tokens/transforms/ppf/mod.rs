#[cfg(feature = "test_access")]
pub mod test;

use std::{borrow::Cow, path::Path};

use crate::{
    api::processor::file_io::read_utf8_file,
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{errors::TextForgeError, validations::check_vec_len},
};

/// `ppf {file};` — Prepend the contents of `file`.
///
/// Inserts the contents of a UTF-8 file at the beginning of the current text,
/// the counterpart of [`Apf`](super::apf::Apf). A missing file or invalid
/// UTF-8 is an error. The path may use `{{ }}`/`[[ ]]` interpolation; a
/// relative path is relative to the working directory. The file is read each
/// time the instruction runs. See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::ppf::Ppf};
///
/// let path = std::env::temp_dir().join(format!("ppf-doc-{}.txt", std::process::id()));
/// std::fs::write(&path, "🍌 ").unwrap();
/// let token = Ppf::new(&path.to_string_lossy());
/// assert_eq!(token.transform("banana".into(), None).unwrap(), "🍌 banana");
/// # std::fs::remove_file(&path).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Ppf {
    path: String,
    params: Vec<TextForgeParamTypes>,
}

impl Ppf {
    /// Builds `ppf {file};` with the given parameters.
    pub fn new(path: &str) -> Self {
        Ppf {
            path: path.to_owned(),
            params: vec![path.to_owned().into()],
        }
    }
}

impl InstructionMethods for Ppf {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "ppf"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("ppf {};\n", self.path).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let mut prefix = read_utf8_file(Path::new(&self.path), "")?;
        if prefix.is_empty() {
            return Ok(input);
        }
        if input.is_empty() {
            return Ok(Cow::Owned(prefix));
        }
        // The file's own buffer becomes the output: one copy of the text.
        prefix.push_str(&input);
        Ok(Cow::Owned(prefix))
    }

    /// Prepends without loading a spooled document into memory.
    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        let prefix = read_utf8_file(Path::new(&self.path), "")?;
        if prefix.is_empty() {
            return Ok(input);
        }
        input.prepend(&prefix)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        check_vec_len(params, 1, "ppf", "")?;
        self.path = parse_args!(params, 0, String, "path");
        self.params = vec![self.path.clone().into()];
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x59
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        Ok(to_bytecode!(
            self.get_opcode(),
            [TextForgeParamTypes::String(self.path.clone())]
        ))
    }
}
