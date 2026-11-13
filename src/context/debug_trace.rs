//! Nested step trace for debug processing.
//!
//! Only debug runs enable it (`GlobalExecutionContext::enable_trace`). Container
//! instructions (`cblk`, `iter`, `on*`) and replacement maps (`rmape`) report
//! what happens inside them; the debug processor drains the rendered lines after
//! each top-level step. The trace only reads documents, so it can never change
//! a result. When it is disabled, every hook is a single `Option` check.
use std::borrow::Cow;

use colored::Colorize;

use crate::{context::execution_context::GlobalExecutionContext, utils::errors::TextForgeError};

/// Maximum replacements listed per `rmape` call; the rest are summarized.
pub(crate) const MAX_LISTED_REPLACEMENTS: usize = 20;

/// Maximum characters shown for each replaced snippet.
const MAX_SNIPPET_CHARS: usize = 40;

/// Size of a document as shown in the trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Size {
    pub chars: usize,
    pub lines: usize,
}

impl Size {
    pub(crate) fn of(text: &str) -> Self {
        Self {
            chars: text.chars().count(),
            lines: crate::utils::lines::lines(text).count(),
        }
    }
}

/// One replacement made by a replacement map, recorded from the single pass
/// over the original text (nothing is re-applied to describe it).
pub(crate) struct Replacement {
    /// One-based rule index, in definition order.
    pub rule: usize,
    pub pattern: String,
    pub original: String,
    pub replacement: String,
    /// Character offsets of the original match.
    pub start: usize,
    pub end: usize,
}

/// Maximum nested steps listed per container; the rest are summarized as
/// `… +K steps` and are neither rendered nor measured.
pub(crate) const MAX_LISTED_STEPS: usize = 20;

/// One open container.
struct Frame {
    /// Nested steps finished so far.
    steps: usize,
    /// Nothing inside this container is rendered (it is past a parent's limit).
    hidden: bool,
}

/// Rendered nested trace lines plus the open containers.
#[derive(Default)]
pub(crate) struct DebugTrace {
    lines: Vec<String>,
    frames: Vec<Frame>,
}

impl DebugTrace {
    fn indent(&self) -> String {
        "  ".repeat(self.frames.len())
    }

    /// Whether the next nested step (or container) at the current position
    /// will be rendered. Callers skip measuring documents when it will not.
    pub(crate) fn will_show(&self) -> bool {
        self.frames
            .last()
            .is_none_or(|frame| !frame.hidden && frame.steps < MAX_LISTED_STEPS)
    }

    /// Open a container. `Some(header)` renders `▶ {header}`; `None` opens a
    /// hidden container (callers pass `None` when [`Self::will_show`] is false).
    pub(crate) fn enter(&mut self, header: Option<&str>) {
        if let Some(header) = header {
            let line = format!("{}{} {}", self.indent(), "▶".cyan(), header.yellow());
            self.lines.push(line);
        }
        self.frames.push(Frame {
            steps: 0,
            hidden: header.is_none(),
        });
    }

    /// Close the innermost container: `◀ {name}  {summary}` when it is visible,
    /// preceded by `… +K steps` when it ran more steps than were listed.
    pub(crate) fn leave(&mut self, name: &str, sizes: Option<(Size, Size)>, extra: &str) {
        let Some(frame) = self.frames.pop() else {
            return;
        };
        let Some((before, after)) = sizes.filter(|_| !frame.hidden) else {
            return;
        };
        if frame.steps > MAX_LISTED_STEPS {
            let rest = frame.steps - MAX_LISTED_STEPS;
            let line = format!("{}  │ … +{rest} {}", self.indent(), plural(rest, "step"));
            self.lines.push(line);
        }
        let line = format!(
            "{}{} {}  {}{}",
            self.indent(),
            "◀".cyan(),
            name.yellow(),
            net_effect(before, after),
            extra
        );
        self.lines.push(line);
    }

    /// Mark the current position, so a caller can tell whether a nested step
    /// rendered its own container lines.
    pub(crate) fn mark(&self) -> usize {
        self.lines.len()
    }

    /// Count one nested step of the innermost container. `shown` carries the
    /// instruction and character counts when [`Self::will_show`] was true
    /// before the step. A step that was itself a container (it rendered lines
    /// after `mark`) only advances the counter.
    pub(crate) fn step(&mut self, mark: usize, shown: Option<(&str, usize, usize)>) {
        let Some(frame) = self.frames.last_mut() else {
            return;
        };
        frame.steps += 1;
        let number = frame.steps;
        let Some((instruction, before, after)) = shown else {
            return;
        };
        if self.lines.len() != mark {
            return;
        }
        let line = format!(
            "{}│ {number}. {}  {before} → {after} chars",
            self.indent(),
            instruction.trim_end_matches([';', '\n']).yellow()
        );
        self.lines.push(line);
    }

    /// Describe one replacement-map execution: header, at most
    /// [`MAX_LISTED_REPLACEMENTS`] entries, a count of the rest, and the summary.
    /// Callers only call it when [`Self::will_show`] is true.
    pub(crate) fn replace_map(
        &mut self,
        name: &str,
        rules: usize,
        replacements: &[Replacement],
        total: usize,
        before: Size,
        after: Size,
    ) {
        self.enter(Some(&format!(
            "rmape {name}  ({rules} {})",
            plural(rules, "rule")
        )));
        let indent = self.indent();
        for item in replacements.iter().take(MAX_LISTED_REPLACEMENTS) {
            self.lines.push(format!(
                "{indent}│ rule {} /{}/  '{}' → '{}'  @ {}..{}",
                item.rule,
                item.pattern,
                snippet(&item.original).red(),
                snippet(&item.replacement).green(),
                item.start,
                item.end
            ));
        }
        if total > MAX_LISTED_REPLACEMENTS {
            self.lines.push(format!(
                "{indent}│ … +{} {}",
                total - MAX_LISTED_REPLACEMENTS,
                plural(total - MAX_LISTED_REPLACEMENTS, "replacement")
            ));
        }
        let extra = format!(" · {total} {}", plural(total, "replacement"));
        self.leave(name, Some((before, after)), &extra);
    }

    /// A one-line remark about the current step (e.g. a file `wtf` wrote),
    /// shown when the step itself would be shown.
    pub(crate) fn note(&mut self, text: &str) {
        if self.will_show() {
            let line = format!("{}│ {}", self.indent(), text.dimmed());
            self.lines.push(line);
        }
    }

    /// Rendered lines since the last call, leaving open containers untouched.
    pub(crate) fn take_lines(&mut self) -> Vec<String> {
        std::mem::take(&mut self.lines)
    }
}

/// Run a container instruction's body inside `▶ header` / `◀ name` trace lines.
/// Without tracing this is just `run(input, context)`; `header` and the size
/// computations only run while debugging.
pub(crate) fn within<'a>(
    context: &mut GlobalExecutionContext,
    header: impl FnOnce(&str) -> String,
    name: &str,
    input: Cow<'a, str>,
    run: impl FnOnce(Cow<'a, str>, &mut GlobalExecutionContext) -> Result<Cow<'a, str>, TextForgeError>,
) -> Result<Cow<'a, str>, TextForgeError> {
    let Some(trace) = context.trace_mut() else {
        return run(input, context);
    };
    // Past a parent's step limit the container is hidden: no header, no sizes.
    let header = trace.will_show().then(|| header(&input));
    let before = header.as_ref().map(|_| Size::of(&input));
    trace.enter(header.as_deref());
    let output = run(input, context)?;
    if let Some(trace) = context.trace_mut() {
        trace.leave(name, before.map(|before| (before, Size::of(&output))), "");
    }
    Ok(output)
}

fn net_effect(before: Size, after: Size) -> String {
    let delta = after.chars as i128 - before.chars as i128;
    let sign = if delta > 0 {
        "+"
    } else if delta < 0 {
        "−"
    } else {
        "±"
    };
    format!(
        "{} → {} chars ({sign}{}) · {} → {} {}",
        before.chars,
        after.chars,
        delta.unsigned_abs(),
        before.lines,
        after.lines,
        plural(after.lines, "line")
    )
}

pub(crate) fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        word.to_owned()
    } else {
        format!("{word}s")
    }
}

/// Escape control characters and shorten long snippets so one replacement
/// always renders on one line.
fn snippet(text: &str) -> String {
    let mut out = String::new();
    for (index, c) in text.chars().enumerate() {
        if index == MAX_SNIPPET_CHARS {
            out.push('…');
            break;
        }
        out.extend(c.escape_debug());
    }
    out
}

/// Debug snapshot shared by the buffered and unbuffered processor tests.
#[cfg(test)]
pub(crate) mod fixtures {
    /// A pipeline exercising every traced container.
    pub(crate) const NESTED_DEBUG_PIPELINE: &str = "blk cleanup assoc tbs;
    blk cleanup assoc raw '  ' ' ';
    blk cleanup assoc atb '# ';
    cblk cleanup;
    rmapd animals add cat wolf;
    rmapd animals add wolf dog;
    rmape animals;
    iter 2 times ate '[[__COUNTER__]]';
    onln 1 apply tua;
    ";

    /// Expected nested trace of [`NESTED_DEBUG_PIPELINE`], shared with the
    /// unbuffered debug snapshot so both modes stay identical.
    pub(crate) const NESTED_DEBUG_TRACE: &[&str] = &[
        "▶ cblk cleanup  (3 instructions)\n  │ 1. tbs  16 → 14 chars\n  │ 2. raw       14 → 12 chars\n  │ 3. atb #   12 → 14 chars\n◀ cleanup  16 → 14 chars (−2) · 2 → 2 lines\n\n",
        "▶ rmape animals  (2 rules)\n  │ rule 1 /cat/  'cat' → 'wolf'  @ 2..5\n  │ rule 2 /wolf/  'wolf' → 'dog'  @ 7..11\n◀ animals  14 → 14 chars (±0) · 2 → 2 lines · 2 replacements\n\n",
        "▶ iter 2 times\n  │ 1. ate 0  14 → 15 chars\n  │ 2. ate 1  15 → 16 chars\n◀ iter  14 → 16 chars (+2) · 2 → 2 lines\n\n",
        "▶ onln 1  (line: 12 chars)\n  │ 1. tua  12 → 12 chars\n◀ onln  16 → 16 chars (±0) · 2 → 2 lines\n\n",
    ];
}

#[cfg(test)]
mod test {
    use super::*;

    fn plain(lines: Vec<String>) -> Vec<String> {
        let ansi = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
        lines
            .into_iter()
            .map(|line| ansi.replace_all(&line, "").into_owned())
            .collect()
    }

    #[test]
    fn nested_containers_indent_and_number_their_steps() {
        let mut trace = DebugTrace::default();
        let before = Size { chars: 5, lines: 1 };
        trace.enter(Some("cblk b  (2 instructions)"));
        let mark = trace.mark();
        trace.step(mark, Some(("tbs;\n", 5, 3)));
        let mark = trace.mark();
        trace.enter(Some("iter 2 times"));
        let inner = trace.mark();
        trace.step(inner, Some(("ate x;\n", 3, 4)));
        let one = |chars| Size { chars, lines: 1 };
        trace.leave("iter", Some((one(3), one(4))), "");
        trace.step(mark, Some(("iter 2 times", 3, 4)));
        trace.leave("b", Some((before, one(4))), "");
        assert_eq!(
            plain(trace.take_lines()),
            vec![
                "▶ cblk b  (2 instructions)",
                "  │ 1. tbs  5 → 3 chars",
                "  ▶ iter 2 times",
                "    │ 1. ate x  3 → 4 chars",
                "  ◀ iter  3 → 4 chars (+1) · 1 → 1 line",
                "◀ b  5 → 4 chars (−1) · 1 → 1 line",
            ]
        );
    }

    #[test]
    fn replacement_lists_are_limited_and_snippets_escaped() {
        let mut trace = DebugTrace::default();
        let items: Vec<_> = (0..25)
            .map(|i| Replacement {
                rule: 1,
                pattern: "a".into(),
                original: "a\n".into(),
                replacement: "b".repeat(50),
                start: i,
                end: i + 1,
            })
            .collect();
        let size = Size {
            chars: 25,
            lines: 1,
        };
        trace.replace_map("m", 1, &items, 25, size, size);
        let lines = plain(trace.take_lines());
        assert_eq!(lines.len(), 1 + MAX_LISTED_REPLACEMENTS + 2);
        assert_eq!(lines[0], "▶ rmape m  (1 rule)");
        assert_eq!(
            lines[1],
            format!("  │ rule 1 /a/  'a\\n' → '{}…'  @ 0..1", "b".repeat(40))
        );
        assert_eq!(lines[21], "  │ … +5 replacements");
        assert_eq!(
            lines[22],
            "◀ m  25 → 25 chars (±0) · 1 → 1 line · 25 replacements"
        );
    }
}
