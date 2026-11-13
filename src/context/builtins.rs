//! Built-in read-only variables and the single variable lookup used by the DSL.
//!
//! Static built-ins come from the [`StaticContext`](super::static_context::StaticContext).
//! Text-derived built-ins (`__LEN_CHARS__`, `__LINES__`) are never stored: they
//! are computed when a parameter reads them, from the text the running
//! instruction receives, at most once per parameter resolution. Pipelines that
//! never read them pay nothing.
use std::{borrow::Cow, cell::OnceCell};

use super::execution_context::{GlobalContextMethods, GlobalExecutionContext, VarValues};
use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

/// A built-in variable. Its name is reserved: `var`, `val`, `mutv` and `eval`
/// cannot define it, and it is read-only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builtin {
    PipelineName,
    TextforgeVersion,
    InputFile,
    LenChars,
    Lines,
}

impl Builtin {
    pub(crate) const ALL: [Builtin; 5] = [
        Builtin::PipelineName,
        Builtin::TextforgeVersion,
        Builtin::InputFile,
        Builtin::LenChars,
        Builtin::Lines,
    ];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Builtin::PipelineName => "__PIPELINE_NAME__",
            Builtin::TextforgeVersion => "__TEXTFORGE_VERSION__",
            Builtin::InputFile => "__INPUT_FILE__",
            Builtin::LenChars => "__LEN_CHARS__",
            Builtin::Lines => "__LINES__",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Builtin> {
        if !name.starts_with("__") {
            return None;
        }
        Builtin::ALL
            .into_iter()
            .find(|builtin| builtin.name() == name)
    }
}

/// Reject a DSL definition (`var`, `val`, `mutv`, `eval … in`) of a built-in name.
pub(crate) fn check_definable(name: &str, instruction: &'static str) -> Result<(), TextForgeError> {
    match Builtin::from_name(name) {
        Some(builtin) => Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("`{}` is a read-only built-in variable", builtin.name()).into(),
            ),
            instruction,
            name.to_owned(),
        )),
        None => Ok(()),
    }
}

#[cfg(test)]
thread_local! {
    /// Number of text scans for derived built-ins (tests only).
    static DERIVED_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The text the running instruction receives, for text-derived built-ins.
pub(crate) enum CurrentText<'a> {
    /// No text is being processed (e.g. serializing a pipeline).
    Unavailable,
    Str(&'a str),
    #[cfg(feature = "buffered")]
    Buffer(std::cell::RefCell<&'a mut crate::api::TextBuffer>),
}

/// Reads variables for one parameter resolution: stored variables from the
/// execution context, built-ins from the static context or the current text.
pub(crate) struct VarReader<'c, 't> {
    context: &'c GlobalExecutionContext,
    text: CurrentText<'t>,
    len_chars: OnceCell<usize>,
    lines: OnceCell<usize>,
}

impl<'c, 't> VarReader<'c, 't> {
    pub(crate) fn new(context: &'c GlobalExecutionContext, text: CurrentText<'t>) -> Self {
        VarReader {
            context,
            text,
            len_chars: OnceCell::new(),
            lines: OnceCell::new(),
        }
    }

    pub(crate) fn context(&self) -> &'c GlobalExecutionContext {
        self.context
    }

    /// The value of `name`: a built-in or a stored variable.
    pub(crate) fn value(&self, name: &str) -> Result<Cow<'c, VarValues>, TextForgeError> {
        match Builtin::from_name(name) {
            Some(builtin) => self.builtin(builtin).map(Cow::Owned),
            None => self
                .context
                .get_var(name)
                .map(|entry| Cow::Borrowed(&entry.value)),
        }
    }

    fn builtin(&self, builtin: Builtin) -> Result<VarValues, TextForgeError> {
        let statics = self.context.static_context();
        Ok(match builtin {
            Builtin::PipelineName => VarValues::String(statics.pipeline_name().to_owned()),
            Builtin::TextforgeVersion => VarValues::String(statics.textforge_version().to_owned()),
            // A non-UTF-8 path is converted lossily: the value is text.
            Builtin::InputFile => VarValues::String(
                statics
                    .input_file()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Builtin::LenChars => VarValues::Usize(self.derived(&self.len_chars, builtin)?),
            Builtin::Lines => VarValues::Usize(self.derived(&self.lines, builtin)?),
        })
    }

    fn derived(&self, cache: &OnceCell<usize>, builtin: Builtin) -> Result<usize, TextForgeError> {
        if let Some(value) = cache.get() {
            return Ok(*value);
        }
        #[cfg(test)]
        DERIVED_SCANS.with(|scans| scans.set(scans.get() + 1));
        let mut stats = TextStats::default();
        match &self.text {
            CurrentText::Unavailable => {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::RequiredContextError(
                        "This built-in variable is only available while text is processed".into(),
                    ),
                    "builtin variable",
                    builtin.name(),
                ));
            }
            CurrentText::Str(text) if builtin == Builtin::LenChars => {
                stats.feed_chars(text.as_bytes());
            }
            CurrentText::Str(text) => stats.feed_lines(text.as_bytes()),
            #[cfg(feature = "buffered")]
            CurrentText::Buffer(buffer) => buffer.borrow_mut().for_each_chunk(|chunk| {
                if builtin == Builtin::LenChars {
                    stats.feed_chars(chunk);
                } else {
                    stats.feed_lines(chunk);
                }
            })?,
        }
        let value = match builtin {
            Builtin::LenChars => stats.chars,
            _ => stats.lines(),
        };
        Ok(*cache.get_or_init(|| value))
    }
}

/// Character and logical-line counts over UTF-8 bytes fed in arbitrary chunks.
///
/// Lines follow `docs/lines.md` (`utils::lines`): empty text has zero lines, a
/// final terminator belongs to its line, CRLF is one terminator, and `\n`,
/// `\r`, VT, FF, NEL, LS and PS are terminators. Working on bytes lets a chunk
/// boundary fall inside a code point. Each count is fed separately, so a
/// pipeline reading only one of them scans only for that one.
#[derive(Default)]
struct TextStats {
    chars: usize,
    breaks: usize,
    seen_text: bool,
    /// The last three bytes fed so far, oldest first, for terminators that
    /// span a chunk boundary and for checking how the text ends.
    previous: [u8; 3],
}

impl TextStats {
    fn feed_chars(&mut self, bytes: &[u8]) {
        // Every byte that is not a UTF-8 continuation byte starts a character.
        self.chars += count_bytes(bytes, |byte| (byte as i8) >= -0x40);
    }

    fn feed_lines(&mut self, bytes: &[u8]) {
        let Some(&first) = bytes.first() else {
            return;
        };
        // Every terminator byte counts once, except the LF of a CRLF; NEL, LS
        // and PS are looked for only when their last byte occurs at all.
        let mut breaks = count_bytes(bytes, |byte| byte.wrapping_sub(0x0A) < 4);
        breaks -= count_pairs(bytes, |a, b| (a == b'\r') & (b == b'\n'));
        if bytes.iter().any(|&byte| matches!(byte, 0x85 | 0xA8 | 0xA9)) {
            breaks += count_pairs(bytes, |a, b| (a == 0xC2) & (b == 0x85));
            breaks += count_triples(bytes, |a, b, c| (a == 0xE2) & (b == 0x80) & (c | 1 == 0xA9));
        }
        // Terminators that started in the previous chunk.
        let [_, before, last] = self.previous;
        let second = bytes.get(1).copied();
        if last == b'\r' && first == b'\n' {
            breaks -= 1;
        }
        let completes = (last == 0xC2 && first == 0x85)
            || (before == 0xE2 && last == 0x80 && matches!(first, 0xA8 | 0xA9))
            || (last == 0xE2 && first == 0x80 && matches!(second, Some(0xA8 | 0xA9)));
        breaks += usize::from(completes);

        self.breaks += breaks;
        let previous = self.previous;
        self.previous = match *bytes {
            [last] => [previous[1], previous[2], last],
            [before, last] => [previous[2], before, last],
            [.., third, before, last] => [third, before, last],
            [] => previous,
        };
        self.seen_text = true;
    }

    fn ends_with_break(&self) -> bool {
        let [third, before, last] = self.previous;
        matches!(last, 0x0A..=0x0D)
            || (last == 0x85 && before == 0xC2)
            || (matches!(last, 0xA8 | 0xA9) && before == 0x80 && third == 0xE2)
    }

    fn lines(&self) -> usize {
        self.breaks + usize::from(self.seen_text && !self.ends_with_break())
    }
}

// Counting helpers. Each counts in blocks of 255 bytes with a `u8` counter and
// a branch-free predicate, a shape the compiler vectorizes: about 6 to 10 ms
// per 100 MB, against 30 to 40 ms for `filter().count()` or `windows()`
// (`docs/benchmarks/builtin-variables/README.md`).
const BLOCK: usize = u8::MAX as usize;

/// Bytes satisfying `matches`.
fn count_bytes(bytes: &[u8], matches: impl Fn(u8) -> bool) -> usize {
    bytes
        .chunks(BLOCK)
        .map(|block| {
            block
                .iter()
                .fold(0u8, |count, &byte| count + u8::from(matches(byte))) as usize
        })
        .sum()
}

/// Adjacent byte pairs satisfying `matches`.
fn count_pairs(bytes: &[u8], matches: impl Fn(u8, u8) -> bool) -> usize {
    let Some(end) = bytes.len().checked_sub(1) else {
        return 0;
    };
    bytes[..end]
        .chunks(BLOCK)
        .zip(bytes[1..].chunks(BLOCK))
        .map(|(first, second)| {
            first
                .iter()
                .zip(second)
                .fold(0u8, |count, (&a, &b)| count + u8::from(matches(a, b))) as usize
        })
        .sum()
}

/// Adjacent byte triples satisfying `matches`.
fn count_triples(bytes: &[u8], matches: impl Fn(u8, u8, u8) -> bool) -> usize {
    let Some(end) = bytes.len().checked_sub(2) else {
        return 0;
    };
    bytes[..end]
        .chunks(BLOCK)
        .zip(bytes[1..=end].chunks(BLOCK))
        .zip(bytes[2..].chunks(BLOCK))
        .map(|((first, second), third)| {
            first
                .iter()
                .zip(second)
                .zip(third)
                .fold(0u8, |count, ((&a, &b), &c)| {
                    count + u8::from(matches(a, b, c))
                }) as usize
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::lines::lines;

    fn line_count(text: &str, chunk: usize) -> (usize, usize) {
        let mut stats = TextStats::default();
        for part in text.as_bytes().chunks(chunk.max(1)) {
            stats.feed_chars(part);
            stats.feed_lines(part);
        }
        (stats.chars, stats.lines())
    }

    #[test]
    fn counts_match_the_shared_line_scanner_for_any_chunk_size() {
        let samples = [
            "",
            "a",
            "\n",
            "a\n",
            "a\r\nb",
            "a\r\r\nb\n\r",
            "\r\n\r\n",
            "x\u{85}y\u{2028}z\u{2029}",
            "é\u{b}ü\u{c}👩🏽‍💻\n",
            "e\u{301}\r",
            "line1\nline2\nline3",
            "\u{2028}",
            // U+1028 (E1 80 A8) and U+0828 share final bytes with LS (E2 80 A8).
            "a\u{1028}",
            "\u{1028}\u{2029}\u{828}",
            "\u{2029}\u{1029}",
        ];
        for text in samples {
            for chunk in 1..=6 {
                assert_eq!(
                    line_count(text, chunk),
                    (text.chars().count(), lines(text).count()),
                    "text {text:?}, chunk {chunk}"
                );
            }
        }
    }

    #[test]
    fn counts_match_the_shared_line_scanner_on_long_mixed_text() {
        let pieces = [
            "a",
            "\r\n",
            "\r",
            "\n",
            "é",
            "\u{85}",
            "\u{2028}",
            "\u{2029}",
            "\u{1028}",
            "\u{b}",
            "\u{c}",
            "👩🏽‍💻",
            "e\u{301}",
            "\u{828}",
            "xyz",
        ];
        // A deterministic sequence that puts every pair of pieces side by side.
        let mut text = String::new();
        let mut state = 7u32;
        for _ in 0..4_000 {
            state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            text.push_str(pieces[(state >> 16) as usize % pieces.len()]);
        }
        for ending in ["", "\r", "\u{2028}", "a"] {
            let text = format!("{text}{ending}");
            let expected = (text.chars().count(), lines(&text).count());
            for chunk in [1, 2, 3, 5, 254, 255, 256, 1_000, text.len()] {
                assert_eq!(line_count(&text, chunk), expected, "chunk {chunk}");
            }
        }
    }

    fn scans() -> usize {
        DERIVED_SCANS.with(std::cell::Cell::get)
    }

    #[test]
    fn derived_values_are_computed_only_when_read_and_at_most_once() {
        use crate::utils::expr::interpolate_with;
        let context = GlobalExecutionContext::new();

        let reader = VarReader::new(&context, CurrentText::Str("a\nb\n"));
        let before = scans();
        assert_eq!(
            interpolate_with("plain [[1 + 1]]", &reader).unwrap(),
            "plain 2"
        );
        assert_eq!(scans(), before, "no built-in read: no scan");

        let reader = VarReader::new(&context, CurrentText::Str("a\nb\n"));
        let text = "{{__LINES__}} {{__LINES__}} [[__LINES__ * 10]]";
        assert_eq!(interpolate_with(text, &reader).unwrap(), "2 2 20");
        assert_eq!(scans(), before + 1, "three reads, one scan");

        assert_eq!(interpolate_with("{{__LEN_CHARS__}}", &reader).unwrap(), "4");
        assert_eq!(scans(), before + 2);
    }

    #[test]
    fn derived_values_need_the_current_text_and_static_values_do_not() {
        use crate::utils::{errors::TextForgeErrorCode, expr::interpolate};
        let context = GlobalExecutionContext::new();
        let error = interpolate("{{__LINES__}}", &context).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::RequiredContextError(_)
        ));
        assert_eq!(
            interpolate("v{{__TEXTFORGE_VERSION__}}|{{__INPUT_FILE__}}|", &context).unwrap(),
            format!("v{}||", env!("CARGO_PKG_VERSION"))
        );
    }

    #[cfg(feature = "buffered")]
    #[test]
    fn spooled_buffers_are_counted_without_materializing() {
        use crate::api::{TextBuffer, TextBufferOptions};
        let text = "é\r\n👩🏽‍💻\u{2028}x".repeat(30_000);
        let mut buffer =
            TextBuffer::from_text(&text, TextBufferOptions::new(16, std::env::temp_dir())).unwrap();
        assert!(buffer.is_spilled());
        let context = GlobalExecutionContext::new();
        {
            let reader = VarReader::new(
                &context,
                CurrentText::Buffer(std::cell::RefCell::new(&mut buffer)),
            );
            let lines = reader.value("__LINES__").unwrap().into_owned();
            let chars = reader.value("__LEN_CHARS__").unwrap().into_owned();
            assert!(matches!(lines, VarValues::Usize(n) if n == lines_of(&text)));
            assert!(matches!(chars, VarValues::Usize(n) if n == text.chars().count()));
        }
        assert!(buffer.is_spilled());
        assert_eq!(buffer.into_string().unwrap(), text);
    }

    #[cfg(feature = "buffered")]
    fn lines_of(text: &str) -> usize {
        lines(text).count()
    }

    #[test]
    fn builtin_names_are_recognized_exactly() {
        for builtin in Builtin::ALL {
            assert_eq!(Builtin::from_name(builtin.name()), Some(builtin));
        }
        assert_eq!(Builtin::from_name("__COUNTER__"), None);
        assert_eq!(Builtin::from_name("__lines__"), None);
        assert_eq!(Builtin::from_name("LINES"), None);
    }
}
