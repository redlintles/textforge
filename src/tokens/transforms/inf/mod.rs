#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    api::processor::file_io::{read_utf8_file, resolve_path},
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::{InstructionMethods, transforms::ins::insert_after_char},
    utils::{errors::TextForgeError, validations::check_vec_len},
};

/// `inf {index} {file};` — Insert the contents of `file` at the start (index 0) or after the character at `index`.
///
/// Inserts the contents of a UTF-8 file into the current text, with exactly the
/// index rules and the argument order of `ins {index} {text}`: index 0 is the
/// start; any other index inserts after the character at that position (base
/// 0); an index equal to the character count appends; a larger one is an
/// error. Up to 0.3.1 the order was `inf {file} {index}`. The file is read first, so a file error is
/// reported before an index error. The path may use
/// `{{ }}`/`[[ ]]` interpolation; a relative path is resolved from the
/// directory of the pipeline file (the working directory for pipelines built
/// in code). See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::inf::Inf};
///
/// let path = std::env::temp_dir().join(format!("inf-doc-{}.txt", std::process::id()));
/// std::fs::write(&path, " orange").unwrap();
/// let token = Inf::new(1, &path.to_string_lossy());
/// assert_eq!(token.transform("banana".into(), None).unwrap(), "ba orangenana");
/// # std::fs::remove_file(&path).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Inf {
    path: String,
    index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Inf {
    /// Builds `inf {index} {file};` with the given parameters.
    pub fn new(index: usize, path: &str) -> Self {
        Inf {
            path: path.to_owned(),
            index,
            params: vec![index.into(), path.to_owned().into()],
        }
    }
}

impl InstructionMethods for Inf {
    fn supports_preparation(&self) -> bool {
        true
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }

    fn get_string_repr(&self) -> &'static str {
        "inf"
    }

    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("inf {} {};\n", self.index, self.path).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let inserted = read_utf8_file(&resolve_path(&self.path, context.as_deref()), "")?;
        insert_after_char(input, self.index, &inserted, || self.to_textforge_line())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        check_vec_len(params, 2, "inf", "")?;
        self.index = parse_args!(params, 0, Usize, "index");
        self.path = parse_args!(params, 1, String, "path");
        self.params = vec![self.index.into(), self.path.clone().into()];
        Ok(())
    }

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x57
    }

    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        Ok(to_bytecode!(
            self.get_opcode(),
            [
                TextForgeParamTypes::Usize(self.index),
                TextForgeParamTypes::String(self.path.clone()),
            ]
        ))
    }
}
