use std::borrow::Cow;

use crate::context::execution_context::GlobalExecutionContext;
use crate::utils::errors::TextForgeError;

use crate::parser::params::TextForgeParamTypes;

pub mod instructions;
pub mod transforms;

/// InstructionMethods
///
/// Basic Contract which every token should implement
pub trait InstructionMethods: InstructionMethodsClone + Send + Sync {
    /// to_textforge_line
    ///
    /// Converts the token to an ATP line to be written in an .textforge file
    fn to_textforge_line(&self) -> Cow<'static, str>;
    /// transform
    ///
    /// Responsible for applying the respective token transformation to `input`
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError>;

    /// Experimental whole-document execution. The compatibility implementation
    /// materializes the input; individual tokens can override this for bounded I/O.
    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        let options = input.options().clone();
        let output = self.transform(input.into_string()?.into(), context)?;
        crate::api::TextBuffer::from_text(output.as_ref(), options)
    }

    /// get_string_repr
    ///
    /// Converts the token to a string representation without parameters, to be used in the mappings
    fn get_string_repr(&self) -> &'static str;

    fn from_params(&mut self, params: &Vec<TextForgeParamTypes>) -> Result<(), TextForgeError>;

    fn get_params(&self) -> &Vec<TextForgeParamTypes>;
    /// BytecodeMethods
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError>;

    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32;
}

pub trait InstructionMethodsClone {
    fn clone_box(&self) -> Box<dyn InstructionMethods>;
}

impl<T> InstructionMethodsClone for T
where
    T: 'static + InstructionMethods + Clone,
{
    fn clone_box(&self) -> Box<dyn InstructionMethods> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn InstructionMethods> {
    fn clone(&self) -> Box<dyn InstructionMethods> {
        self.clone_box()
    }
}
