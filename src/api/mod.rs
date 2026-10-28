#[cfg(feature = "buffered")]
mod text_buffer;
#[cfg(feature = "buffered")]
pub use text_buffer::{TextBuffer, TextBufferOptions};

pub mod block_builder;
pub mod builder;
pub mod conditional_builder;
pub mod processor;

use crate::api::block_builder::BlockBuilder;
use crate::api::conditional_builder::ConditionalBuilderEach;
use crate::parser::params::TextForgeParamTypes;
use crate::parser::resolve_var::{ TokenWrapper, ValType };
use crate::tokens::{ InstructionMethods, instructions::*, transforms::* };
use crate::utils::errors::TextForgeError;

// Helper for referencing variables in code
pub fn get_var(var_name: &str) -> ValType {
    ValType::VarRef(var_name.to_string())
}
pub trait TextForgeBuilderMethods: Sized {
    fn push_token(&mut self, t: impl Into<TokenWrapper>) -> Result<(), TextForgeError>;

    /// TBS - Trim Both Sides
    ///
    /// Removes whitespace characters from both the left and right sides of the input.
    ///
    /// See Also:
    ///
    /// - [`Tls` - Trim Left Side](crate::tokens::transforms::tls)
    /// - [`Trs` - Trim Right Side](crate::tokens::transforms::trs)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    ///
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).trim_both_sides().unwrap().build();
    /// let input = "   banana   ";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("banana".to_string()));
    /// ```
    fn trim_both_sides(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(tbs::Tbs::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// TLS - Trim Left Side
    ///
    /// Removes whitespace characters exclusively from the left side of the input.
    ///
    /// See Also:
    ///
    /// - [`Trs` - Trim Right Side](crate::tokens::transforms::trs)
    /// - [`Tbs` - Trim Both Sides](crate::tokens::transforms::tbs)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).trim_left_side().unwrap().build();
    /// let input = "   banana  ";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("banana  ".to_string()));
    /// ```
    fn trim_left_side(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(tls::Tls::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// TRS - Trim Right Side
    ///
    /// Removes whitespace characters exclusively from the right side of the input.
    ///
    /// See Also:
    ///
    /// - [`Tls` - Trim Left Side](crate::tokens::transforms::tls)
    /// - [`Tbs` - Trim Both Sides](crate::tokens::transforms::tbs)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).trim_right_side().unwrap().build();
    /// let input = "  banana   ";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("  banana".to_string()));
    /// ```
    fn trim_right_side(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(trs::Trs::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// ATE - Add To End
    ///
    /// Appends the provided `text` to the end of the input string.
    ///
    /// See Also:
    ///
    /// - [`Atb` - Add To Beginning](crate::tokens::transforms::atb)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).add_to_end("!").unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("banana!".to_string()));
    /// ```
    fn add_to_end(&mut self, text: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(ate::Ate::default()), Some(vec![text.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// ATB - Add To Beginning
    ///
    /// Inserts the provided `text` at the beginning of the input string.
    ///
    /// See Also:
    ///
    /// - [`Ate` - Add To End](crate::tokens::transforms::ate)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).add_to_beginning("x").unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("xbanana".to_string()));
    /// ```

    fn add_to_beginning(&mut self, text: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(atb::Atb::default()), Some(vec![text.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// DLF - Delete First
    ///
    /// Removes the first character of the input string.
    ///
    /// See Also:
    ///
    /// - [`Dll` - Delete Last](crate::tokens::transforms::dll)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).delete_first().unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("anana".to_string()));
    /// ```

    fn delete_first(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(dlf::Dlf::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// DLL - Delete Last
    ///
    /// Removes the last character of the input string.
    ///
    /// See Also:
    ///
    /// - [`Dlf` - Delete First](crate::tokens::transforms::dlf)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).delete_last().unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("banan".to_string()));
    /// ```

    fn delete_last(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(dll::Dll::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// DLA - Delete After
    ///
    /// Removes all characters after the provided `index`, keeping the content from `0..=index`.
    ///
    /// See Also:
    ///
    /// - [`Dlb` - Delete Before](crate::tokens::transforms::dlb)
    /// - [`Dlc` - Delete Chunk](crate::tokens::transforms::dlc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).delete_after(2).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("ban".to_string()));
    /// ```

    fn delete_after(&mut self, index: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(dla::Dla::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// DLB - Delete Before
    ///
    /// Removes all characters before the provided `index`, keeping the content from `index..`.
    ///
    /// See Also:
    ///
    /// - [`Dla` - Delete After](crate::tokens::transforms::dla)
    /// - [`Dlc` - Delete Chunk](crate::tokens::transforms::dlc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).delete_before(3).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("ana".to_string()));
    /// ```

    fn delete_before(&mut self, index: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(dlb::Dlb::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// DLC - Delete Chunk
    ///
    /// Removes all characters between `start_index` and `end_index` (inclusive),
    /// effectively slicing out that segment from the input string.
    ///
    /// See Also:
    ///
    /// - [`Dla` - Delete After](crate::tokens::transforms::dla)
    /// - [`Dlb` - Delete Before](crate::tokens::transforms::dlb)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).delete_chunk(1, 3).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id, input), Ok("bna".to_string()));
    /// ```

    fn delete_chunk(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(dlc::Dlc::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RAW - Replace All With
    ///
    /// Replaces **all** occurrences of `pattern` with `text_to_replace`.
    ///
    /// See Also:
    ///
    /// - [`Replace First`](crate::tokens::transforms::rfw)
    /// - [`Replace Last`](crate::tokens::transforms::rlw)
    /// - [`Replace Nth`](crate::tokens::transforms::rnw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).replace_all_with("a", "x").unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("bxnxnx".to_string())
    /// );
    /// ```

    fn replace_all_with(
        &mut self,
        pattern: impl Into<ValType>,
        text_to_replace: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(raw::Raw::default()),
            Some(vec![pattern.into(), text_to_replace.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RFW - Replace First With
    ///
    /// Replaces only the **first** occurrence of `pattern` with `text_to_replace`.
    ///
    /// See Also:
    ///
    /// - [`Replace All`](crate::tokens::transforms::raw)
    /// - [`Replace Last`](crate::tokens::transforms::rlw)
    /// - [`Replace Nth`](crate::tokens::transforms::rnw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id =
    ///     TextForgeBuilder::new(&mut processor).replace_first_with("a", "x").unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("bxnana".to_string())
    /// );
    /// ```

    fn replace_first_with(
        &mut self,
        pattern: impl Into<ValType>,
        text_to_replace: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(rfw::Rfw::default()),
            Some(vec![pattern.into(), text_to_replace.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RLW - Replace Last With
    ///
    /// Replaces only the **last** occurrence of `pattern` with `text_to_replace`.
    ///
    /// See Also:
    ///
    /// - [`Replace First`](crate::tokens::transforms::rfw)
    /// - [`Replace All`](crate::tokens::transforms::raw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).replace_last_with("a", "x").unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("bananx".to_string())
    /// );
    /// ```

    fn replace_last_with(
        &mut self,
        pattern: impl Into<ValType>,
        text_to_replace: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(rlw::Rlw::default()),
            Some(vec![pattern.into(), text_to_replace.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RNW - Replace Nth With
    ///
    /// Replaces the **nth** occurrence (0-based) of `pattern`
    /// with `text_to_replace`. If the index does not exist, no changes occur.
    ///
    /// See Also:
    ///
    /// - [`Replace All`](crate::tokens::transforms::raw)
    /// - [`Replace First`](crate::tokens::transforms::rfw)
    /// - [`Replace Last`](crate::tokens::transforms::rlw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).replace_nth_with("a", "x", 1).unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("banxna".to_string())
    /// );
    /// ```

    fn replace_nth_with(
        &mut self,
        pattern: impl Into<ValType>,
        text_to_replace: impl Into<ValType>,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(rnw::Rnw::default()),
            Some(vec![pattern.into(), text_to_replace.into(), index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RCW - Replace Count With
    ///
    /// Replaces up to **count** occurrences of `pattern` with `text_to_replace`,
    /// scanning from left to right.
    ///
    /// See Also:
    ///
    /// - [`Replace All`](crate::tokens::transforms::raw)
    /// - [`Replace Nth`](crate::tokens::transforms::rnw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).replace_count_with("a", "x", 2).unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("bxnxna".to_string())
    /// );
    /// ```

    fn replace_count_with(
        &mut self,
        pattern: impl Into<ValType>,
        text_to_replace: impl Into<ValType>,
        count: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(rcw::Rcw::default()),
            Some(vec![pattern.into(), text_to_replace.into(), count.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RTL - Rotate Left
    ///
    /// Rotates the characters of the input to the **left** `times` positions.
    ///
    /// `"abcd".rotate_left(1)` → `"bcda"`
    ///
    /// See Also:
    ///
    /// - [`Rotate Right`](crate::tokens::transforms::rtr)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).rotate_left(2).unwrap().build();
    ///
    /// let input = "abcd";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("cdab".to_string())
    /// );
    /// ```

    fn rotate_left(&mut self, times: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(rtl::Rtl::default()), Some(vec![times.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// RTR - Rotate Right
    ///
    /// Rotates the characters of the input to the **right** `times` positions.
    ///
    /// `"abcd".rotate_right(1)` → `"dabc"`
    ///
    /// See Also:
    ///
    /// - [`Rotate Left`](crate::tokens::transforms::rtl)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).rotate_right(1).unwrap().build();
    ///
    /// let input = "abcd";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("dabc".to_string())
    /// );
    /// ```

    fn rotate_right(&mut self, times: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(rtr::Rtr::default()), Some(vec![times.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// RPT - Repeat
    ///
    /// Repeats the entire input string `times` times.
    ///
    /// See Also:
    ///
    /// - [`Pad Right`](crate::tokens::transforms::padr)
    /// - [`Pad Left`](crate::tokens::transforms::padl)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).repeat(3).unwrap().build();
    ///
    /// let input = "hi";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("hihihi".to_string())
    /// );
    /// ```

    fn repeat(&mut self, times: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(rpt::Rpt::default()), Some(vec![times.into()]));
        self.push_token(tok)?;
        Ok(self)
    }

    /// SLT - Select
    ///
    /// Extracts a substring from `start_index` to `end_index` (inclusive).
    ///
    /// See Also:
    ///
    /// - [`Delete Chunk`](crate::tokens::transforms::dlc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).select(1, 3).unwrap().build();
    ///
    /// let input = "abcdef";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("bcd".to_string())
    /// );
    /// ```

    fn select(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(slt::Slt::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }

    /// TUA - To Uppercase All
    ///
    /// Converts all characters of the input string to uppercase.
    ///
    /// See Also:
    ///
    /// - [`To Lowercase All`](crate::tokens::transforms::tla)
    /// - [`To Uppercase Chunk`](crate::tokens::transforms::tucc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).to_uppercase_all().unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("BANANA".to_string())
    /// );
    /// ```

    fn to_uppercase_all(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(tua::Tua::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// TLA - To Lowercase All
    ///
    /// Converts all characters of the input string to lowercase.
    ///
    /// See Also:
    ///
    /// - [`To Uppercase All`](crate::tokens::transforms::tua)
    /// - [`To Lowercase Chunk`](crate::tokens::transforms::tlcc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor).to_lowercase_all().unwrap().build();
    ///
    /// let input = "BaNaNa";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("banana".to_string())
    /// );
    /// ```

    fn to_lowercase_all(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(tla::Tla::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// TUCS - To Uppercase Single
    ///
    /// Converts only the character at `index` to uppercase.
    /// If the index is out of range, no character is modified.
    ///
    /// See Also:
    ///
    /// - [`To Lowercase Single`](crate::tokens::transforms::tlcs)
    /// - [`To Uppercase Chunk`](crate::tokens::transforms::tucc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).to_uppercase_single(1).unwrap().build();
    ///
    /// let input = "banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("bAnana".to_string())
    /// );
    /// ```

    fn to_uppercase_single(
        &mut self,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(tucs::Tucs::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// TLCS - To Lowercase Single
    ///
    /// Converts only the character at `index` to lowercase.
    /// If the index is out of range, no character is modified.
    ///
    /// See Also:
    ///
    /// - [`To Uppercase Single`](crate::tokens::transforms::tucs)
    /// - [`To Lowercase Chunk`](crate::tokens::transforms::tlcc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).to_lowercase_single(0).unwrap().build();
    ///
    /// let input = "Banana";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("banana".to_string())
    /// );
    /// ```

    fn to_lowercase_single(
        &mut self,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(tlcs::Tlcs::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// TUCC - To Uppercase Chunk
    ///
    /// Converts a substring between `start_index` and `end_index` (inclusive)
    /// to uppercase.
    /// Returns an error if the indices are invalid.
    ///
    /// See Also:
    ///
    /// - [`To Lowercase Chunk`](crate::tokens::transforms::tlcc)
    /// - [`To Uppercase All`](crate::tokens::transforms::tua)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_uppercase_chunk(1, 3)
    ///     .unwrap().build();
    ///
    ///
    /// let input = "abcdef";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("aBCDef".to_string())
    /// );
    /// ```

    fn to_uppercase_chunk(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(tucc::Tucc::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// TLCC - To Lowercase Chunk
    ///
    /// Converts a substring between `start_index` and `end_index` (inclusive)
    /// to lowercase.
    /// Returns an error if the indices are invalid.
    ///
    /// See Also:
    ///
    /// - [`To Uppercase Chunk`](crate::tokens::transforms::tucc)
    /// - [`To Lowercase All`](crate::tokens::transforms::tla)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_lowercase_chunk(2, 5)
    ///     .unwrap().build();
    ///
    ///
    /// let input = "ABCD EF";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("ABcd eF".to_string())
    /// );
    /// ```

    fn to_lowercase_chunk(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(tlcc::Tlcc::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }

    /// CFW - Capitalize First Word
    ///
    /// Capitalizes the **first word** of the input string.
    /// A "word" is defined as the first contiguous sequence of non-whitespace characters.
    ///
    /// See Also:
    ///
    /// - [`Capitalize Last Word`](crate::tokens::transforms::clw) // expected token name
    /// - [`Capitalize Chunk`](crate::tokens::transforms::ctc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id =
    ///     TextForgeBuilder::new(&mut processor).capitalize_first_word().unwrap().build();
    ///
    /// let input = "hello world";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("Hello world".to_string())
    /// );
    /// ```

    fn capitalize_first_word(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(cfw::Cfw::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// CLW - Capitalize Last Word
    ///
    /// Capitalizes the **last word** of the input string.
    ///
    /// See Also:
    ///
    /// - [`Capitalize First Word`](crate::tokens::transforms::cfw)
    /// - [`Capitalize Chunk`](crate::tokens::transforms::ctc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();

    /// let id =
    ///     TextForgeBuilder::new(&mut processor).capitalize_last_word().unwrap().build();
    ///
    /// let input = "hello world";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("hello World".to_string())
    /// );
    /// ```

    fn capitalize_last_word(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(clw::Clw::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// SSLT - Split Select
    ///
    /// Splits the input string using `pattern` and selects the part at `index`.
    /// If the index does not exist, returns an empty string.
    ///
    /// See Also:
    ///
    /// - [`Select`](crate::tokens::transforms::slt)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id =
    ///     TextForgeBuilder::new(&mut processor).split_select("-", 1).unwrap().build();
    ///
    /// let input = "aa-bb-cc";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("bb".to_string())
    /// );
    /// ```

    fn split_select(
        &mut self,
        pattern: impl Into<ValType>,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(sslt::Sslt::default()),
            Some(vec![pattern.into(), index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// CTC - Capitalize Chunk
    ///
    /// Capitalizes the substring between `start_index` and `end_index` (inclusive).
    /// Returns an error if the indices are invalid.
    ///
    /// See Also:
    ///
    /// - [`Capitalize First Word`](crate::tokens::transforms::cfw)
    /// - [`Capitalize Last Word`](crate::tokens::transforms::clw)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .capitalize_chunk(6, 16)
    ///     .unwrap().build();
    ///
    /// let input = "hello brave world";
    ///
    /// assert_eq!(
    ///     processor.process_all(&id,&input),
    ///     Ok("hello Brave World".to_string())
    /// );
    /// ```

    fn capitalize_chunk(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(ctc::Ctc::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// CTR - Capitalize Range
    ///
    /// Capitalizes all characters in `input` from `start_index` (inclusive) to `end_index`
    /// (exclusive).
    /// If the indices are invalid, an `TextForgeError` is returned at build-time.
    ///
    /// See Also:
    ///
    /// - [`Ctc` - Capitalize Chunk](crate::tokens::transforms::ctc)
    /// - [`Cts` - Capitalize Single Word](crate::tokens::transforms::cts)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .capitalize_range(1, 4)
    ///     .unwrap().build(); // required because this method returns Result
    ///
    ///
    /// let input = "foo bar mar jar tar rar";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("foo Bar Mar Jar Tar rar".to_string())
    /// );
    /// ```
    fn capitalize_range(
        &mut self,
        start_index: impl Into<ValType>,
        end_index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(ctr::Ctr::default()),
            Some(vec![start_index.into(), end_index.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// CTS - Capitalize Single Word
    ///
    /// Capitalizes the word located at the given `index` in `input`.
    /// Words are delimited according to Unicode whitespace rules.
    ///
    /// See Also:
    ///
    /// - [`Cfw` - Capitalize First Word](crate::tokens::transforms::cfw)
    /// - [`Ctc` - Capitalize Chunk](crate::tokens::transforms::ctc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .capitalize_single_word(2)
    ///     .unwrap().build();
    ///
    /// let input = "hello brave world";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("hello brave World".to_string())
    /// );
    /// ```
    fn capitalize_single_word(
        &mut self,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(cts::Cts::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// URLE - URL Encode
    ///
    /// Converts the entire `input` string into its URL-encoded form
    /// according to RFC 3986 percent-encoding rules.
    ///
    /// See Also:
    ///
    /// - [`Urld` - URL Decode](crate::tokens::transforms::urld)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_url_encoded()
    ///     .unwrap().build();
    ///
    /// let input = "hello world!";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("hello%20world%21".to_string())
    /// );
    /// ```

    fn to_url_encoded(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(urle::Urle::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// URLD - URL Decode
    ///
    /// Decodes a URL-encoded string into its normal representation.
    /// Invalid percent-encoded sequences remain unchanged.
    ///
    /// See Also:
    ///
    /// - [`Urle` - URL Encode](crate::tokens::transforms::urle)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_url_decoded()
    ///     .unwrap().build();
    ///
    /// let input = "hello%20world%21";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("hello world!".to_string())
    /// );
    /// ```

    fn to_url_decoded(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(urld::Urld::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// REV - Reverse Text
    ///
    /// Reverses all characters in the input string.
    ///
    /// This operation is Unicode-aware and preserves grapheme clusters.
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_reverse()
    ///     .unwrap().build();
    ///
    /// let input = "abc";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("cba".to_string())
    /// );
    /// ```
    fn to_reverse(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(rev::Rev::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// SPLC - Split Characters
    ///
    /// Splits the entire input string into individual characters separated by spaces.
    /// Grapheme clusters are preserved (Unicode-aware).
    ///
    /// Example: `"abc"` → `"a b c"`
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .split_characters()
    ///     .unwrap().build();
    ///
    /// let input = "hello";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("h e l l o".to_string())
    /// );
    /// ```

    fn split_characters(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(splc::Splc::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// HTMLE - HTML Escape
    ///
    /// Escapes HTML special characters such as `<`, `>`, `"`, `'`, `&`.
    /// Useful for preventing HTML injection or rendering raw text.
    ///
    /// See Also:
    ///
    /// - [`Htmlu` - HTML Unescape](crate::tokens::transforms::htmlu)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_html_escaped()
    ///     .unwrap().build();
    ///
    /// let input = "<b>Hello</b>";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("&lt;b&gt;Hello&lt;&#x2F;b&gt;".to_string())
    /// );
    /// ```

    fn to_html_escaped(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(htmle::Htmle::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// HTMLU - HTML Unescape
    ///
    /// Converts HTML escaped entities back into their literal characters.
    /// Example: `"&lt;" → "<"`
    ///
    /// See Also:
    ///
    /// - [`Htmle` - HTML Escape](crate::tokens::transforms::htmle)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor)
    ///     .to_html_unescaped()
    ///     .unwrap().build();
    ///
    /// let input = "&lt;b&gt;Hi&lt;/b&gt;";
    /// assert_eq!(
    ///     processor.process_all(&id, &input),
    ///     Ok("<b>Hi</b>".to_string())
    /// );
    /// ```
    fn to_html_unescaped(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(htmlu::Htmlu::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// To Json Escaped
    ///
    /// Escapes JSON characters of `string``
    ///
    /// See Also:
    ///
    /// - [JSONU - To json unescaped](crate::tokens::transforms::jsonu)
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).to_json_escaped().unwrap().build();
    /// let input = "{banana: '10'}";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("\"{banana: '10'}\"".to_string()));
    /// ```

    fn to_json_escaped(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jsone::Jsone::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// To Json Unescaped
    ///
    /// Unescapes JSON characters of `string``
    ///
    /// See Also:
    ///
    /// - [JSONE - To json escaped](crate::tokens::transforms::jsone)
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).to_json_unescaped().unwrap().build();
    /// let input = "\"{banana: '10'}\"";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("{banana: '10'}".to_string()));
    /// ```
    fn to_json_unescaped(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jsonu::Jsonu::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// Insert
    ///
    /// Inserts `text` after `index` of `string`
    ///
    /// See Also:
    ///
    /// - [ATB - Add to Beginning](crate::tokens::transforms::atb)
    /// - [ATE - Add to End](crate::tokens::transforms::ate)
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).insert(1, " laranja").unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("ba laranjanana".to_string()));
    /// ```
    fn insert(
        &mut self,
        index: impl Into<ValType>,
        text_to_insert: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(ins::Ins::default()),
            Some(vec![index.into(), text_to_insert.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }

    /// To Lowercase Word
    ///
    /// Lowercases a single word of `string`
    ///
    /// See Also:
    ///
    /// - [TUCW - To Uppercase Word](crate::tokens::transforms::tucw)
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).to_lowercase_word(1).unwrap().build();
    /// let input = "BANANA LARANJA CHEIA DE CANJA";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("BANANA laranja CHEIA DE CANJA".to_string()));
    /// ```
    fn to_lowercase_word(
        &mut self,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(tlcw::Tlcw::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }
    /// To Uppercase Word
    ///
    /// Uppercases a single word of `string`
    ///
    /// See Also:
    ///
    /// - [TLCW - To Lowercase Word](crate::tokens::transforms::tlcw)
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).to_uppercase_word(1).unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banana LARANJA cheia de canja".to_string()));
    /// ```
    fn to_uppercase_word(
        &mut self,
        index: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(tucw::Tucw::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }

    /// Join to kebab-case
    ///
    /// If `input` is a string whose words are separated by spaces, join `input` as a lowercased kebab-case string
    ///
    /// See Also:
    ///
    /// - [`Jpsc` - Join to Pascal Case](crate::tokens::transforms::jpsc)
    /// - [`Jsnc` - Join to Snake Case](crate::tokens::transforms::jsnc)
    /// - [`Jcmc` - Join to Camel Case](crate::tokens::transforms::jcmc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).join_to_kebab_case().unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banana-laranja-cheia-de-canja".to_string()));
    ///

    fn join_to_kebab_case(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jkbc::Jkbc::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// Join to snake_case
    ///
    /// If `input` is a string whose words are separated by spaces, join `input` as a lowercased snake_case string
    ///
    /// See Also:
    ///
    /// - [`Jpsc` - Join to Pascal Case](crate::tokens::transforms::jpsc)
    /// - [`Jkbc` - Join to Kebab Case](crate::tokens::transforms::jkbc)
    /// - [`Jcmc` - Join to Camel Case](crate::tokens::transforms::jcmc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).join_to_snake_case().unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banana_laranja_cheia_de_canja".to_string()));
    ///
    fn join_to_snake_case(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jsnc::Jsnc::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// Join to camelCase
    ///
    /// If `input` is a string whose words are separated by spaces, join `input` as a camelCase string
    ///
    /// See Also:
    ///
    /// - [`Jpsc` - Join to Pascal Case](crate::tokens::transforms::jpsc)
    /// - [`Jsnc` - Join to Snake Case](crate::tokens::transforms::jsnc)
    /// - [`Jkbc` - Join to Kebab Case](crate::tokens::transforms::jkbc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).join_to_camel_case().unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("bananaLaranjaCheiaDeCanja".to_string()));
    /// ```
    fn join_to_camel_case(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jcmc::Jcmc::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// Join to PascalCase
    ///
    /// If `input` is a string whose words are separated by spaces, join `input` as a camelCase string
    ///
    /// See Also:
    ///
    /// - [`Jsnc` - Join to Snake Case](crate::tokens::transforms::jsnc)
    /// - [`Jcmc` - Join to Camel Case](crate::tokens::transforms::jcmc)
    /// - [`Jkbc` - Join to Kebab Case](crate::tokens::transforms::jkbc)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).join_to_pascal_case().unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("BananaLaranjaCheiaDeCanja".to_string()));
    /// ```
    fn join_to_pascal_case(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(jpsc::Jpsc::default());
        self.push_token(tok)?;
        Ok(self)
    }
    /// PADL - Pad Left
    ///
    /// Repeats `text` characters until `max_len` is reached, and then insert the result at the start of `input`
    ///
    /// See Also:
    ///
    /// - [`Padr` - Pad Left](crate::tokens::transforms::padr)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).pad_left("x", 7).unwrap().build();
    /// let input = "banana";
    ///
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("xbanana".to_string()));
    /// ```
    fn pad_left(
        &mut self,
        text: impl Into<ValType>,
        times: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(padl::Padl::default()),
            Some(vec![text.into(), times.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// PADR - Pad Right
    ///
    /// Repeats `text` characters until `max_len` is reached, and then insert the result at the end of `input`
    ///
    /// See Also:
    ///
    /// - [`Padl` - Pad Left](crate::tokens::transforms::padl)
    ///
    /// # Example:
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).pad_right("x", 7).unwrap().build();
    /// let input = "banana";
    ///
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("bananax".to_string()));
    /// ```
    fn pad_right(
        &mut self,
        text: impl Into<ValType>,
        times: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(padr::Padr::default()),
            Some(vec![text.into(), times.into()])
        );
        self.push_token(tok)?;
        Ok(self)
    }
    /// RMWS - Remove Whitespace
    ///
    /// Removes all whitespaces in `input`
    ///
    /// # Example:
    ///
    /// /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).remove_whitespace().unwrap().build();
    /// let input = "banana laranja cheia de canja";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("bananalaranjacheiadecanja".to_string()));
    /// ```
    fn remove_whitespace(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok: Box<dyn InstructionMethods> = Box::new(rmws::Rmws::default());
        self.push_token(tok)?;
        Ok(self)
    }

    /// DLS - Delete Single
    ///
    /// Delete's a single character specified by `index` in `input`
    ///
    /// It will throw an `TextForgeError` if index does not exists in `input`
    ///
    /// # Example:
    ///
    /// ```rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).delete_single(3).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banna".to_string()));
    /// ```
    fn delete_single(&mut self, index: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(dls::Dls::default()), Some(vec![index.into()]));
        self.push_token(tok)?;
        Ok(self)
    }

    /// EMJ - Extract_matches_joined
    ///
    /// Extract all non-overlapping matches in input and join them with a separator, discarding the rest of the `input`
    ///
    /// It will return an empty string if no matches are found
    ///
    /// # Example:
    ///
    /// ``` rust
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).extract_matches_joined("a", ",").unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("a,a,a".to_string()));
    ///
    /// ```
    fn extract_matches_joined(
        &mut self,
        pattern: impl Into<ValType>,
        separator: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(emj::Emj::default()),
            Some(vec![pattern.into(), separator.into()])
        );

        self.push_token(tok)?;

        Ok(self)
    }

    /// VAL - Immutable Variable Declaration
    ///
    /// Store's an Immutable Variable in GlobalExecutionContext
    ///
    /// # Example:
    ///
    /// ```rust
    ///
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    ///     get_var
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).val("x", 3).unwrap().delete_single(get_var("x")).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banna".to_string()));
    /// ```

    fn val(
        &mut self,
        var_name: impl Into<ValType>,
        var_value: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let var_value: ValType = var_value.into();

        let var_value = match var_value {
            ValType::Literal(value) => {
                ValType::Literal(TextForgeParamTypes::String(value.to_string()))
            }
            ValType::VarRef(name) => ValType::VarRef(name),
        };

        let tok = TokenWrapper::new(
            Box::new(val::Val::default()),
            Some(vec![var_name.into(), var_value])
        );

        self.push_token(tok)?;
        Ok(self)
    }
    /// VAR - mutable Variable Declaration
    ///
    /// Store's an mutable Variable in GlobalExecutionContext
    ///
    /// # Example:
    ///
    /// ```rust
    ///
    /// use textforge::api::{
    ///     builder::TextForgeBuilder,
    ///     processor::{TextForgeProcessor,TextForgeProcessorMethods},
    ///     TextForgeBuilderMethods,
    ///     get_var
    /// };
    /// let mut processor = TextForgeProcessor::new();
    /// let id = TextForgeBuilder::new(&mut processor).var("x", 3).unwrap().delete_single(get_var("x")).unwrap().build();
    /// let input = "banana";
    ///
    /// assert_eq!(processor.process_all(&id,&input), Ok("banna".to_string()));
    /// ```

    fn var(
        &mut self,
        var_name: impl Into<ValType>,
        var_value: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let var_value: ValType = var_value.into();

        let var_value = match var_value {
            ValType::Literal(value) => {
                ValType::Literal(TextForgeParamTypes::String(value.to_string()))
            }
            ValType::VarRef(name) => ValType::VarRef(name),
        };

        let tok = TokenWrapper::new(
            Box::new(var::Var::default()),
            Some(vec![var_name.into(), var_value])
        );

        self.push_token(tok)?;
        Ok(self)
    }
    fn mut_var(
        &mut self,
        var_name: impl Into<ValType>,
        var_value: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let var_value: ValType = var_value.into();

        let var_value = match var_value {
            ValType::Literal(value) => {
                ValType::Literal(TextForgeParamTypes::String(value.to_string()))
            }
            ValType::VarRef(name) => ValType::VarRef(name),
        };

        let tok = TokenWrapper::new(
            Box::new(mutv::Mutv::default()),
            Some(vec![var_name.into(), var_value])
        );

        self.push_token(tok)?;
        Ok(self)
    }

    fn eval(
        &mut self,
        expr: impl Into<ValType>,
        var_name: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(eval::Eval::default()),
            Some(vec![expr.into(), var_name.into()])
        );

        self.push_token(tok)?;
        Ok(self)
    }

    fn iter(
        &mut self,
        times: impl Into<ValType>,
        inner: impl Into<ValType>
    ) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(
            Box::new(iter::Iter::default()),
            Some(vec![times.into(), inner.into()])
        );

        self.push_token(tok)?;
        Ok(self)
    }
    fn remove_pattern(&mut self, pattern: impl Into<ValType>) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(rmp::Rmp::default()), Some(vec![pattern.into()]));

        self.push_token(tok)?;
        Ok(self)
    }
    fn end(&mut self) -> Result<&mut Self, TextForgeError> {
        let tok = TokenWrapper::new(Box::new(end::End::default()), Some(vec![]));

        self.push_token(tok)?;
        Ok(self)
    }
}

pub trait TextForgeConditionalMethods: TextForgeBuilderMethods {
    fn if_do_contains_each<F>(&mut self, value: &str, f: F) -> Result<&mut Self, TextForgeError>
        where F: FnOnce(&mut ConditionalBuilderEach) -> Result<(), TextForgeError>
    {
        let params = vec![TextForgeParamTypes::String(value.to_string())];
        let token: Box<dyn InstructionMethods> = Box::new(ifdc::Ifdc::default());
        let mut conditional_builder = ConditionalBuilderEach::new(token, params);

        f(&mut conditional_builder)?;

        let result = conditional_builder.build();

        for token in result.into_iter() {
            self.push_token(token)?;
        }

        Ok(self)
    }
}

pub trait TextForgeBlockMethods: TextForgeBuilderMethods {
    fn block_assoc<F>(
        &mut self,
        block_name: &'static str,
        f: F
    ) -> Result<&mut Self, TextForgeError>
        where F: FnOnce(&mut BlockBuilder) -> Result<(), TextForgeError>
    {
        let mut block_builder = BlockBuilder::new(block_name);

        f(&mut block_builder)?;

        let result = block_builder.build();

        for token in result.into_iter() {
            self.push_token(token)?;
        }
        Ok(self)
    }

    fn call_block(&mut self, block_name: &'static str) -> Result<&mut Self, TextForgeError> {
        let mut t: Box<dyn InstructionMethods> = Box::new(cblk::Cblk::default());

        t.from_params(&vec![TextForgeParamTypes::String(block_name.to_string())])?;

        self.push_token(t)?;
        Ok(self)
    }
}
