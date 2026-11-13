#[cfg(feature = "test_access")]
pub mod test;

use std::{borrow::Cow, path::Path};

use crate::{
    api::processor::file_io::read_utf8_file,
    context::execution_context::GlobalExecutionContext,
    parser::params::TextForgeParamTypes,
    tokens::{InstructionMethods, transforms::ins::insert_after_char},
    utils::{errors::TextForgeError, validations::check_vec_len},
};

/// Inf - Insert File
///
/// Inserts the contents of a UTF-8 file into the current text, with exactly the
/// index rules of `ins`: after the character at `index` (base 0); an index
/// equal to the character count appends; a larger one is an error. Like `ins`,
/// it cannot insert before the first character. The file is read first, so a
/// file error is reported before an index error. The path may use
/// `{{ }}`/`[[ ]]` interpolation; a relative path is relative to the working
/// directory. See `docs/file-instructions.md`.
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::inf::Inf};
///
/// let path = std::env::temp_dir().join(format!("inf-doc-{}.txt", std::process::id()));
/// std::fs::write(&path, " laranja").unwrap();
/// let token = Inf::new(&path.to_string_lossy(), 1);
/// assert_eq!(token.transform("banana".into(), None).unwrap(), "ba laranjanana");
/// # std::fs::remove_file(&path).unwrap();
/// ```
#[derive(Clone, Default)]
pub struct Inf {
    path: String,
    index: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Inf {
    pub fn new(path: &str, index: usize) -> Self {
        Inf {
            path: path.to_owned(),
            index,
            params: vec![path.to_owned().into(), index.into()],
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
        format!("inf {} {};\n", self.path, self.index).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        let inserted = read_utf8_file(Path::new(&self.path), "inf")?;
        insert_after_char(input, self.index, &inserted, || self.to_textforge_line())
    }

    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;
        check_vec_len(params, 2, "inf", "")?;
        self.path = parse_args!(params, 0, String, "Path should be of string type");
        self.index = parse_args!(params, 1, Usize, "Index should be of usize type");
        self.params = vec![self.path.clone().into(), self.index.into()];
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
                TextForgeParamTypes::String(self.path.clone()),
                TextForgeParamTypes::Usize(self.index),
            ]
        ))
    }
}
