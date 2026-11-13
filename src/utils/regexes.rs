use std::sync::LazyLock;

use regex::Regex;

/// The empty pattern, for the `Default` of regex instructions (cloning shares
/// the compiled program instead of compiling it on every `default()`).
pub(crate) static EMPTY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("").expect("the empty regex is a static valid pattern"));

/// Variable names: a letter or `_`, then letters, digits or `_` — the same
/// identifiers `[[ ]]` expressions accept (e.g. `__COUNTER__`, `line_no`).
pub static VAR_REF_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}$")
        .expect("VAR_REF regex is a static valid pattern")
});
pub static EXPR_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[(.*?)\]\]").expect("EXPR regex is a static valid pattern"));

pub static INTERP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?P<esc>\\(?:\{\{|\}\}|\[\[|\]\]))|\{\{(?P<var>[a-zA-Z_][a-zA-Z0-9_]*)\}\}|\[\[(?P<expr>.*?)\]\]|\{\{|\}\}|\[\[|\]\]"
    ).expect("INTERP regex is a static valid pattern")
});
