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

/// Apf - Append File
///
/// Appends the contents of a UTF-8 file to the end of the current text. A
/// missing file or invalid UTF-8 is an error. The path may use `{{ }}`/`[[ ]]`
/// interpolation; a relative path is relative to the working directory. The
/// file is read each time the instruction runs. See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::apf::Apf};
///
/// let path = std::env::temp_dir().join(format!("apf-doc-{}.txt", std::process::id()));
/// std::fs::write(&path, " 🍌").unwrap();
/// let token = Apf::new(&path.to_string_lossy());
/// assert_eq!(token.transform("banana".into(), None).unwrap(), "banana 🍌");
/// # std::fs::remove_file(&path).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Apf {
    path: String,
    params: Vec<TextForgeParamTypes>,
}

impl Apf {
    pub fn new(path: &str) -> Self {
        Apf {
            path: path.to_owned(),
            params: vec![path.to_owned().into()],
        }
    }
}

impl InstructionMethods for Apf {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "apf"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("apf {};\n", self.path).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let appended = read_utf8_file(Path::new(&self.path), "apf")?;
        if appended.is_empty() {
            return Ok(input);
        }
        // Grow the input's own buffer when it is owned: no extra copy of it.
        let mut output = input.into_owned();
        output.push_str(&appended);
        Ok(Cow::Owned(output))
    }

    /// Appends without loading a spooled document into memory.
    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        mut input: crate::api::TextBuffer,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        let appended = read_utf8_file(Path::new(&self.path), "apf")?;
        input.append_text(&appended)?;
        Ok(input)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        check_vec_len(params, 1, "apf", "")?;
        self.path = parse_args!(params, 0, String, "Path should be of string type");
        self.params = vec![self.path.clone().into()];
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x56
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
