#[cfg(feature = "test_access")]
pub mod test;

use std::{borrow::Cow, io::Write, path::Path};

use crate::{
    api::processor::file_io::write_atomically,
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

/// Wtf - Write to File
///
/// Writes the current text to `path` and returns it **unchanged**, so it can be
/// used in the middle of a pipeline. The file is created or replaced
/// atomically (written to a temporary file in the same directory, then renamed):
/// on failure the previous file is untouched. Missing directories are not
/// created. The path may use `{{ }}`/`[[ ]]` interpolation; a relative path is
/// relative to the working directory. See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::wtf::Wtf};
///
/// let path = std::env::temp_dir().join(format!("wtf-doc-{}.txt", std::process::id()));
/// let token = Wtf::new(&path.to_string_lossy());
/// assert_eq!(token.transform("banana".into(), None).unwrap(), "banana");
/// assert_eq!(std::fs::read_to_string(&path).unwrap(), "banana");
/// # std::fs::remove_file(&path).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Wtf {
    path: String,
    params: Vec<TextForgeParamTypes>,
}

impl Wtf {
    pub fn new(path: &str) -> Self {
        Wtf {
            path: path.to_owned(),
            params: vec![path.to_owned().into()],
        }
    }

    fn written(&self, bytes: u64, context: Option<&mut GlobalExecutionContext>) {
        if let Some(trace) = context.and_then(GlobalExecutionContext::trace_mut) {
            trace.note(&format!("wtf wrote {bytes} bytes to {}", self.path));
        }
    }
}

fn write_error(error: std::io::Error) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::FileWritingError(error.to_string().into()),
        "wtf",
        error.to_string(),
    )
}

impl InstructionMethods for Wtf {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "wtf"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("wtf {};\n", self.path).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        write_atomically(Path::new(&self.path), "wtf", |file| {
            file.write_all(input.as_bytes()).map_err(write_error)
        })?;
        self.written(input.len() as u64, context);
        Ok(input)
    }

    /// Streams a spooled document to the file without loading it into memory.
    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        mut input: crate::api::TextBuffer,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        let mut bytes = 0;
        write_atomically(Path::new(&self.path), "wtf", |file| {
            bytes = input.copy_to(file)?;
            Ok(())
        })?;
        self.written(bytes, context);
        Ok(input)
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        check_vec_len(params, 1, "wtf", "")?;
        self.path = parse_args!(params, 0, String, "Path should be of string type");
        self.params = vec![self.path.clone().into()];
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x55
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
