//! Control instructions: variables, blocks, conditionals, loops, sections,
//! replacement maps, pipeline joins and `end`.
/// `blk {name} assoc {instruction};` — Add an instruction to block `name`.
pub mod blk;
/// `cblk {name};` — Run the instructions of block `name`.
pub mod cblk;
/// `end;` — Stop the pipeline (or the current block or loop).
pub mod end;
/// `eval {expression} in {name};` — Evaluate an expression into a variable.
pub mod eval;
/// `ifdc {text} do {instruction};` — Run `instruction` if the text contains `text`.
pub mod ifdc;
/// `iter {times} times {instruction};` — Run `instruction` `times` times (`__COUNTER__` counts from 0).
pub mod iter;
/// `mutv {name} = {value};` — Change a mutable variable.
pub mod mutv;
/// `null;` — Do nothing.
pub mod null;
/// `onck {start} {end} apply {instruction};` — Run `instruction` on the characters `start..=end` only.
pub mod onck;
/// `onln {line} apply {instruction};` — Run `instruction` on one line only.
pub mod onln;
/// `onlni {start} {end} apply {instruction};` — Run `instruction` on lines `start..=end` as one section.
pub mod onlni;
/// `ppjn {pipeline};` — Run the pipeline in another file on the text.
pub mod ppjn;
/// `rmapd {map} add {pattern} {text};` — Add a rule to replacement map `map`.
pub mod rmapd;
/// `val {name} = {value};` — Define an immutable variable.
pub mod val;
/// `var {name} = {value};` — Define a mutable variable.
pub mod var;
