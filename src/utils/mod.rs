//! Errors and helpers. [`errors`] is the part most users need; the other
//! modules are support code for the engine and for custom instructions.
#[doc(hidden)]
pub mod apply;
#[cfg(feature = "cli")]
pub mod cli;
pub mod errors;

#[doc(hidden)]
pub mod expr;
#[doc(hidden)]
pub mod regexes;
#[doc(hidden)]
pub mod transforms;
#[doc(hidden)]
pub mod validations;

#[cfg(feature = "test_access")]
#[doc(hidden)]
pub mod test_helpers;

pub(crate) mod lines;

pub(crate) mod allocation;
pub(crate) mod sections;
pub(crate) mod template;
