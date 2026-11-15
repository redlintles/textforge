//! Transforms: instructions that change the text.
/// `apf {file};` — Append the contents of `file`.
pub mod apf;
/// `apl {text};` — Append a line.
pub mod apl;
/// `atb {text};` — Add `text` to the beginning.
pub mod atb;
/// `ate {text};` — Add `text` to the end.
pub mod ate;
/// `cfw;` — Capitalize the first word.
pub mod cfw;
/// `clw;` — Capitalize the last word.
pub mod clw;
/// `ctc {start} {end};` — Capitalize the words in the characters `start..=end`.
pub mod ctc;
/// `ctr {start} {end};` — Capitalize words `start..=end`.
pub mod ctr;
/// `cts {index};` — Capitalize word `index`.
pub mod cts;
/// `dla {index};` — Delete every character after `index`.
pub mod dla;
/// `dlb {index};` — Delete every character before `index`.
pub mod dlb;
/// `dlc {start} {end};` — Delete the characters `start..=end`.
pub mod dlc;
/// `dle;` — Delete empty lines.
pub mod dle;
/// `dlf;` — Delete the first character.
pub mod dlf;
/// `dll;` — Delete the last character.
pub mod dll;
/// `dlm {pattern};` — Delete the lines that match `pattern`.
pub mod dlm;
/// `dln {line};` — Delete `line`.
pub mod dln;
/// `dlna {line};` — Delete the lines after `line`.
pub mod dlna;
/// `dlnb {line};` — Delete the lines before `line`.
pub mod dlnb;
/// `dlni {start} {end};` — Delete lines `start..=end`.
pub mod dlni;
/// `dls {index};` — Delete the character at `index`.
pub mod dls;
/// `emj {pattern} {separator};` — Keep only the matches, joined with `separator`.
pub mod emj;
/// `htmle;` — Escape HTML special characters.
pub mod htmle;
/// `htmlu;` — Unescape HTML entities.
pub mod htmlu;
/// `inf {file} {index};` — Insert the contents of `file` at the start (index 0) or after the character at `index`.
pub mod inf;
/// `inl {line} {text};` — Insert a line before `line` (one past the last appends).
pub mod inl;
/// `ins {index} {text};` — Insert `text` at the start (index 0) or after the character at `index`.
pub mod ins;
/// `jcmc;` — Join the words as camelCase.
pub mod jcmc;
/// `jkbc;` — Join the words as kebab-case.
pub mod jkbc;
/// `joinln {separator};` — Join the lines with `separator`.
pub mod joinln;
/// `jpsc;` — Join the words as PascalCase.
pub mod jpsc;
/// `jsnc;` — Join the words as snake_case.
pub mod jsnc;
/// `jsone;` — Escape as a JSON string.
pub mod jsone;
/// `jsonu;` — Unescape a JSON string.
pub mod jsonu;
/// `klm {pattern};` — Keep the lines that match `pattern`.
pub mod klm;
/// `nln;` — Remove every line terminator.
pub mod nln;
/// `padl {text} {length};` — Pad on the left with `text` up to `length` characters.
pub mod padl;
/// `padr {text} {length};` — Pad on the right with `text` up to `length` characters.
pub mod padr;
/// `pln {text};` — Prepend a line.
pub mod pln;
/// `ppf {file};` — Prepend the contents of `file`.
pub mod ppf;
/// `raw {pattern} {text};` — Replace every match of `pattern` with `text`.
pub mod raw;
/// `rcw {pattern} {text} {count};` — Replace the first `count` matches.
pub mod rcw;
/// `rev;` — Reverse the characters.
pub mod rev;
/// `revln;` — Reverse the order of the lines.
pub mod revln;
/// `rfw {pattern} {text};` — Replace the first match.
pub mod rfw;
/// `rln {line} {text};` — Replace the content of `line`, keeping its terminator.
pub mod rln;
/// `rlw {pattern} {text};` — Replace the last match.
pub mod rlw;
/// `rmape {map};` — Apply a replacement map defined with `rmapd`, without cascading.
pub mod rmape;
/// `rmp {pattern};` — Remove every match of `pattern`.
pub mod rmp;
/// `rmws;` — Remove all whitespace.
pub mod rmws;
/// `rnw {pattern} {text} {n};` — Replace the `n`th match.
pub mod rnw;
/// `rpt {times};` — Repeat the text `times` times.
pub mod rpt;
/// `rtl {times};` — Rotate the characters `times` positions to the left.
pub mod rtl;
/// `rtr {times};` — Rotate the characters `times` positions to the right.
pub mod rtr;
/// `slnc {start} {end};` — Keep only lines `start..=end`.
pub mod slnc;
/// `slt {start} {end};` — Keep only the characters `start..=end`.
pub mod slt;
/// `splc;` — Separate every character with a space.
pub mod splc;
/// `sslt {pattern} {index};` — Split by `pattern` and keep piece `index`.
pub mod sslt;
/// `tbs;` — Trim whitespace on both sides.
pub mod tbs;
/// `tla;` — Lowercase everything.
pub mod tla;
/// `tlcc {start} {end};` — Lowercase the characters `start..=end`.
pub mod tlcc;
/// `tlcs {index};` — Lowercase the character at `index`.
pub mod tlcs;
/// `tlcw {index};` — Lowercase word `index`.
pub mod tlcw;
/// `tls;` — Trim whitespace on the left.
pub mod tls;
/// `trs;` — Trim whitespace on the right.
pub mod trs;
/// `tua;` — Uppercase everything.
pub mod tua;
/// `tucc {start} {end};` — Uppercase the characters `start..=end`.
pub mod tucc;
/// `tucs {index};` — Uppercase the character at `index`.
pub mod tucs;
/// `tucw {index};` — Uppercase word `index`.
pub mod tucw;
/// `uniqln;` — Keep the first occurrence of each line.
pub mod uniqln;
/// `unorm {mode};` — Normalize Unicode to `nfc`, `nfd`, `nfkc` or `nfkd`.
pub mod unorm;
/// `urld;` — URL-decode.
pub mod urld;
/// `urle;` — URL-encode.
pub mod urle;
/// `wtf {file};` — Write the text to `file` and keep it unchanged.
pub mod wtf;
