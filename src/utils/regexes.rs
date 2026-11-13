use std::sync::LazyLock;

use regex::Regex;

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
