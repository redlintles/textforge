//! # TextForge
//!
//! A small language for **text-processing pipelines**: a pipeline is a list of
//! single-purpose instructions (trim, replace, delete lines, insert a file…)
//! applied to a text in order. Pipelines are written in Rust with a fluent
//! builder, in `.textforge` text files, or loaded from compact `.textforgebc`
//! bytecode, and they run on strings, files, batches of files or spooled
//! documents larger than memory.
//!
//! ```
//! use textforge::api::{
//!     TextForgeBuilderMethods,
//!     processor::{TextForgeProcessor, TextForgeProcessorMethods},
//! };
//!
//! let mut processor = TextForgeProcessor::new();
//! let id = processor
//!     .create_pipeline()
//!     .trim_both_sides()?
//!     .replace_all_with("world", "Rust")?
//!     .to_uppercase_all()?
//!     .build()?;
//!
//! assert_eq!(processor.process_all(&id, "  hello world  ")?, "HELLO RUST");
//! # Ok::<(), textforge::utils::errors::TextForgeError>(())
//! ```
//!
//! The same pipeline as a `.textforge` file, one instruction per line:
//!
//! ```
//! use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
//!
//! let path = std::env::temp_dir().join(format!("crate-doc-{}.textforge", std::process::id()));
//! std::fs::write(&path, "tbs;\nrfw world Rust;\ntua;\n").unwrap();
//!
//! let mut processor = TextForgeProcessor::new();
//! let id = processor.get_pipeline_from_file(&path)?;
//! assert_eq!(processor.process_all(&id, "  hello world  ")?, "HELLO RUST");
//! # std::fs::remove_file(&path).unwrap();
//! # Ok::<(), textforge::utils::errors::TextForgeError>(())
//! ```
//!
//! ## Where to start
//!
//! | You want to… | Use |
//! |---|---|
//! | Register and run pipelines | [`TextForgeProcessor`](api::processor::TextForgeProcessor) and its [`TextForgeProcessorMethods`](api::processor::TextForgeProcessorMethods) |
//! | Build a pipeline in Rust | [`create_pipeline`](api::processor::TextForgeProcessor::create_pipeline), then the methods of [`TextForgeBuilderMethods`](api::TextForgeBuilderMethods) |
//! | Look up what an instruction does | [`tokens`]: every mnemonic with its struct and syntax |
//! | Tune memory use and safety limits | [`ExecutionProfile`](api::ExecutionProfile) and [`Limits`](api::Limits) |
//! | Write a binding for another language | [`api::portable`] |
//! | Handle errors | [`TextForgeError`](utils::errors::TextForgeError) and [`TextForgeErrorCode`](utils::errors::TextForgeErrorCode) |
//! | Write your own instruction | [`InstructionMethods`](tokens::InstructionMethods) |
//!
//! ## Concepts
//!
//! - **Transforms** change the text (`tua` uppercases, `rfw` replaces the first
//!   match). **Instructions** control the run: variables (`var`, `val`, `mutv`,
//!   `eval`), blocks (`blk`, `cblk`), conditionals (`ifdc`), loops (`iter`),
//!   sections (`onln`, `onck`, `onlni`), pipeline joins (`ppjn`) and `end`.
//! - **Interpolation**: string parameters may contain `{{name}}` (a variable or
//!   a built-in such as `__LINES__`) and `[[ expression ]]` (arithmetic and
//!   string expressions). Numeric parameters may be written as expressions too.
//! - **Lines** are logical lines: `\n`, `\r\n` and `\r` end a line and are kept.
//!   Characters are Unicode scalar values; indices are base 0, line numbers base 1.
//! - **Profiles** decide how much memory a run may use before spooling to disk
//!   and the limits of instructions that multiply work (`rpt`, `padl`, `padr`,
//!   `iter`). A profile never changes the result; a limit can only make a run fail.
//!
//! ## Cargo features
//!
//! | Feature | Enables |
//! |---|---|
//! | `bytecode` | The `bytecode` module: reading and writing `.textforgebc` pipelines |
//! | `watchers` | The `watchers` module: probes that observe a run and export a JSON report |
//! | `buffered` | `process_*_buffered` and `api::TextBuffer`: documents spooled to disk |
//! | `cli` | The `txf` command-line program (`cargo install textforge --features cli`) |
//!
//! No feature is enabled by default.
//!
//! ## Guides
//!
//! Longer guides live in the repository's
//! [`docs/`](https://github.com/redlintles/textforge/tree/main/docs) folder:
//! [file instructions](https://github.com/redlintles/textforge/blob/main/docs/file-instructions.md),
//! [section instructions](https://github.com/redlintles/textforge/blob/main/docs/on-instructions.md),
//! [execution profiles](https://github.com/redlintles/textforge/blob/main/docs/execution-profiles.md),
//! [execution context and built-in variables](https://github.com/redlintles/textforge/blob/main/docs/execution-context.md),
//! [line semantics](https://github.com/redlintles/textforge/blob/main/docs/lines.md)
//! and the [portable API](https://github.com/redlintles/textforge/blob/main/docs/portable-api.md).
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "test_access"), warn(missing_docs))]

#[cfg(not(feature = "test_access"))]
mod context;
#[cfg(not(feature = "test_access"))]
mod globals;
#[cfg(not(feature = "test_access"))]
mod macros;
#[cfg(not(feature = "test_access"))]
mod parser;
#[cfg(not(feature = "test_access"))]
mod text;

#[cfg(feature = "test_access")]
pub mod context;
#[cfg(feature = "test_access")]
pub mod globals;
#[cfg(feature = "test_access")]
pub mod macros;

#[cfg(feature = "test_access")]
pub mod text;

#[cfg(feature = "test_access")]
pub mod parser;

// Public

pub mod api;
pub mod tokens;
pub mod utils;

// Bytecode
#[cfg(feature = "bytecode")]
pub mod bytecode;

#[cfg(feature = "watchers")]
pub mod watchers;
