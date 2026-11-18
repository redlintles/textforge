//! Every instruction of the language, one module per mnemonic.
//!
//! [`transforms`] change the text; [`instructions`] control the run
//! (variables, blocks, loops, sections, joins). Each module holds one struct
//! that implements [`InstructionMethods`]; the struct's documentation gives the
//! syntax and an example. Pipelines usually reach them through the builder
//! ([`TextForgeBuilderMethods`](crate::api::TextForgeBuilderMethods)) or the
//! text format rather than by constructing them directly.
//!
//! ## Instruction index
//!
//! Every instruction of the language, grouped by what it does. Parameters in
//! braces may be quoted (`'a b'`), and string parameters accept `{{ }}` and
//! `[[ ]]` interpolation. Every index starts at 0 (characters, words and lines), and
//! ranges such as `start..=end` are inclusive.
//! <!-- instruction-index:begin -->
//!
//! ### Add, insert and repeat
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`atb`](transforms::atb::Atb) | `atb {text};` | Add `text` to the beginning. |
//! | [`ate`](transforms::ate::Ate) | `ate {text};` | Add `text` to the end. |
//! | [`ins`](transforms::ins::Ins) | `ins {index} {text};` | Insert `text` after the character at `index` (`0` inserts at the start). |
//! | [`padl`](transforms::padl::Padl) | `padl {text} {length};` | Pad on the left with `text` up to `length` characters. |
//! | [`padr`](transforms::padr::Padr) | `padr {text} {length};` | Pad on the right with `text` up to `length` characters. |
//! | [`rpt`](transforms::rpt::Rpt) | `rpt {times};` | Repeat the text `times` times. |
//! | [`rev`](transforms::rev::Rev) | `rev;` | Reverse the characters. |
//! | [`rtl`](transforms::rtl::Rtl) | `rtl {times};` | Rotate the characters `times` positions to the left. |
//! | [`rtr`](transforms::rtr::Rtr) | `rtr {times};` | Rotate the characters `times` positions to the right. |
//! | [`splc`](transforms::splc::Splc) | `splc;` | Separate every character with a space. |
//! | [`rmws`](transforms::rmws::Rmws) | `rmws;` | Remove all whitespace. |
//!
//! ### Delete
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`dlf`](transforms::dlf::Dlf) | `dlf;` | Delete the first character. |
//! | [`dll`](transforms::dll::Dll) | `dll;` | Delete the last character. |
//! | [`dls`](transforms::dls::Dls) | `dls {index};` | Delete the character at `index`. |
//! | [`dlc`](transforms::dlc::Dlc) | `dlc {start} {end};` | Delete the characters `start..=end`. |
//! | [`dla`](transforms::dla::Dla) | `dla {index};` | Delete every character after `index`. |
//! | [`dlb`](transforms::dlb::Dlb) | `dlb {index};` | Delete every character before `index`. |
//!
//! ### Replace and match (regular expressions)
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`raw`](transforms::raw::Raw) | `raw {pattern} {text};` | Replace every match of `pattern` with `text`. |
//! | [`rfw`](transforms::rfw::Rfw) | `rfw {pattern} {text};` | Replace the first match. |
//! | [`rlw`](transforms::rlw::Rlw) | `rlw {pattern} {text};` | Replace the last match. |
//! | [`rnw`](transforms::rnw::Rnw) | `rnw {pattern} {text} {n};` | Replace the `n`th match. |
//! | [`rcw`](transforms::rcw::Rcw) | `rcw {pattern} {text} {count};` | Replace the first `count` matches. |
//! | [`rmp`](transforms::rmp::Rmp) | `rmp {pattern};` | Remove every match of `pattern`. |
//! | [`emj`](transforms::emj::Emj) | `emj {pattern} {separator};` | Keep only the matches, joined with `separator`. |
//! | [`rmape`](transforms::rmape::Rmape) | `rmape {map};` | Apply a replacement map defined with `rmapd`, without cascading. |
//!
//! ### Trim and select
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`tbs`](transforms::tbs::Tbs) | `tbs;` | Trim whitespace on both sides. |
//! | [`tls`](transforms::tls::Tls) | `tls;` | Trim whitespace on the left. |
//! | [`trs`](transforms::trs::Trs) | `trs;` | Trim whitespace on the right. |
//! | [`slt`](transforms::slt::Slt) | `slt {start} {end};` | Keep only the characters `start..=end`. |
//! | [`sslt`](transforms::sslt::Sslt) | `sslt {pattern} {index};` | Split by `pattern` and keep piece `index`. |
//!
//! ### Case
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`tua`](transforms::tua::Tua) | `tua;` | Uppercase everything. |
//! | [`tla`](transforms::tla::Tla) | `tla;` | Lowercase everything. |
//! | [`tucs`](transforms::tucs::Tucs) | `tucs {index};` | Uppercase the character at `index`. |
//! | [`tlcs`](transforms::tlcs::Tlcs) | `tlcs {index};` | Lowercase the character at `index`. |
//! | [`tucc`](transforms::tucc::Tucc) | `tucc {start} {end};` | Uppercase the characters `start..=end`. |
//! | [`tlcc`](transforms::tlcc::Tlcc) | `tlcc {start} {end};` | Lowercase the characters `start..=end`. |
//! | [`tucw`](transforms::tucw::Tucw) | `tucw {index};` | Uppercase word `index`. |
//! | [`tlcw`](transforms::tlcw::Tlcw) | `tlcw {index};` | Lowercase word `index`. |
//! | [`cfw`](transforms::cfw::Cfw) | `cfw;` | Capitalize the first word. |
//! | [`clw`](transforms::clw::Clw) | `clw;` | Capitalize the last word. |
//! | [`cts`](transforms::cts::Cts) | `cts {index};` | Capitalize word `index`. |
//! | [`ctc`](transforms::ctc::Ctc) | `ctc {start} {end};` | Capitalize the words in the characters `start..=end`. |
//! | [`ctr`](transforms::ctr::Ctr) | `ctr {start} {end};` | Capitalize words `start..=end`. |
//! | [`jkbc`](transforms::jkbc::Jkbc) | `jkbc;` | Join the words as kebab-case. |
//! | [`jsnc`](transforms::jsnc::Jsnc) | `jsnc;` | Join the words as snake_case. |
//! | [`jcmc`](transforms::jcmc::Jcmc) | `jcmc;` | Join the words as camelCase. |
//! | [`jpsc`](transforms::jpsc::Jpsc) | `jpsc;` | Join the words as PascalCase. |
//!
//! ### Encoding and Unicode
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`urle`](transforms::urle::Urle) | `urle;` | URL-encode. |
//! | [`urld`](transforms::urld::Urld) | `urld;` | URL-decode. |
//! | [`htmle`](transforms::htmle::Htmle) | `htmle;` | Escape HTML special characters. |
//! | [`htmlu`](transforms::htmlu::Htmlu) | `htmlu;` | Unescape HTML entities. |
//! | [`jsone`](transforms::jsone::Jsone) | `jsone;` | Escape as a JSON string. |
//! | [`jsonu`](transforms::jsonu::Jsonu) | `jsonu;` | Unescape a JSON string. |
//! | [`unorm`](transforms::unorm::Unorm) | `unorm {mode};` | Normalize Unicode to `nfc`, `nfd`, `nfkc` or `nfkd`. |
//!
//! ### Lines (line numbers start at 0)
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`apl`](transforms::apl::Apl) | `apl {text};` | Append a line. |
//! | [`pln`](transforms::pln::Pln) | `pln {text};` | Prepend a line. |
//! | [`inl`](transforms::inl::Inl) | `inl {line} {text};` | Insert a line before `line` (one past the last appends). |
//! | [`rln`](transforms::rln::Rln) | `rln {line} {text};` | Replace the content of `line`, keeping its terminator. |
//! | [`dln`](transforms::dln::Dln) | `dln {line};` | Delete `line`. |
//! | [`dlna`](transforms::dlna::Dlna) | `dlna {line};` | Delete the lines after `line`. |
//! | [`dlnb`](transforms::dlnb::Dlnb) | `dlnb {line};` | Delete the lines before `line`. |
//! | [`dlni`](transforms::dlni::Dlni) | `dlni {start} {end};` | Delete lines `start..=end`. |
//! | [`slnc`](transforms::slnc::Slnc) | `slnc {start} {end};` | Keep only lines `start..=end`. |
//! | [`dle`](transforms::dle::Dle) | `dle;` | Delete empty lines. |
//! | [`klm`](transforms::klm::Klm) | `klm {pattern};` | Keep the lines that match `pattern`. |
//! | [`dlm`](transforms::dlm::Dlm) | `dlm {pattern};` | Delete the lines that match `pattern`. |
//! | [`revln`](transforms::revln::Revln) | `revln;` | Reverse the order of the lines. |
//! | [`uniqln`](transforms::uniqln::Uniqln) | `uniqln;` | Keep the first occurrence of each line. |
//! | [`joinln`](transforms::joinln::Joinln) | `joinln {separator};` | Join the lines with `separator`. |
//! | [`nln`](transforms::nln::Nln) | `nln;` | Remove every line terminator. |
//!
//! ### Files
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`wtf`](transforms::wtf::Wtf) | `wtf {file};` | Write the text to `file` and keep it unchanged. |
//! | [`apf`](transforms::apf::Apf) | `apf {file};` | Append the contents of `file`. |
//! | [`ppf`](transforms::ppf::Ppf) | `ppf {file};` | Prepend the contents of `file`. |
//! | [`inf`](transforms::inf::Inf) | `inf {index} {file};` | Insert the contents of `file` after the character at `index` (`0` inserts at the start). |
//! | [`ppjn`](instructions::ppjn::Ppjn) | `ppjn {pipeline};` | Run the pipeline in another file on the text. |
//!
//! ### Variables
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`var`](instructions::var::Var) | `var {name} = {value};` | Define a mutable variable. |
//! | [`val`](instructions::val::Val) | `val {name} = {value};` | Define an immutable variable. |
//! | [`mutv`](instructions::mutv::Mutv) | `mutv {name} = {value};` | Change a mutable variable. |
//! | [`eval`](instructions::eval::Eval) | `eval {expression} in {name};` | Evaluate an expression into a variable. |
//!
//! ### Control flow
//!
//! | Instruction | Syntax | Effect |
//! |---|---|---|
//! | [`blk`](instructions::blk::Blk) | `blk {name} assoc {instruction};` | Add an instruction to block `name`. |
//! | [`cblk`](instructions::cblk::Cblk) | `cblk {name};` | Run the instructions of block `name`. |
//! | [`ifdc`](instructions::ifdc::Ifdc) | `ifdc {text} do {instruction};` | Run `instruction` if the text contains `text`. |
//! | [`iter`](instructions::iter::Iter) | `iter {times} times {instruction};` | Run `instruction` `times` times (`__COUNTER__` counts from 0). |
//! | [`onln`](instructions::onln::Onln) | `onln {line} apply {instruction};` | Run `instruction` on one line only. |
//! | [`onck`](instructions::onck::Onck) | `onck {start} {end} apply {instruction};` | Run `instruction` on the characters `start..=end` only. |
//! | [`onlni`](instructions::onlni::Onlni) | `onlni {start} {end} apply {instruction};` | Run `instruction` on lines `start..=end` as one section. |
//! | [`tryi`](instructions::tryi::Tryi) | `tryi {instruction};` | Run `instruction`; if it fails, skip it (text and variables unchanged) and continue. |
//! | [`rmapd`](instructions::rmapd::Rmapd) | `rmapd {map} add {pattern} {text};` | Add a rule to replacement map `map`. |
//! | [`end`](instructions::end::End) | `end;` | Stop the pipeline (or the current block or loop). |
//! | [`null`](instructions::null::Null) | `null;` | Do nothing. |
//!
//! <!-- instruction-index:end -->
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
        input.transform_with(|text| self.transform(text, context))
    }

    /// Whether literal parameters may be bound once for a registered pipeline.
    /// Opting in requires reusable immutable execution state; custom instructions
    /// default to per-execution binding. Dynamic/interpolated parameters are never
    /// prepared, and validation errors remain execution-time errors.
    ///
    /// ```
    /// use textforge::tokens::{InstructionMethods, transforms::atb::Atb};
    /// assert!(Atb::new("prefix").supports_preparation());
    /// ```
    fn supports_preparation(&self) -> bool {
        false
    }

    /// get_string_repr
    ///
    /// Converts the token to a string representation without parameters, to be used in the mappings
    fn get_string_repr(&self) -> &'static str;

    /// Binds already parsed parameters to this instruction.
    ///
    /// Implementations validate the parameter count and types and return
    /// [`TextForgeError`] on failure. Runtime variable resolution is performed by
    /// the processor before this method is called. See [`Self::get_params`].
    ///
    /// ```
    /// use textforge::tokens::{InstructionMethods, transforms::atb::Atb};
    /// let mut token = Atb::new("before");
    /// let replacement = Atb::new("🍌").get_params().clone();
    /// token.set_params(&replacement).unwrap();
    /// assert_eq!(token.transform("x".into(), None).unwrap(), "🍌x");
    /// ```
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError>;

    /// The instruction's current parameters, in syntax order.
    fn get_params(&self) -> &Vec<TextForgeParamTypes>;
    /// BytecodeMethods
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError>;

    /// The instruction's bytecode opcode (unique among instructions).
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32;
}

/// Clones an instruction behind a `Box<dyn InstructionMethods>`; implemented
/// for every `Clone` instruction.
pub trait InstructionMethodsClone {
    /// A boxed copy of the instruction.
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
