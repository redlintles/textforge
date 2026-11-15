//! Splitting what the player types: the instructions of a line and the words
//! of an instruction.
//!
//! This is a light reading of the text syntax (`docs/text-syntax.md`), enough
//! to find instruction names; the engine parses the instructions for real.

/// A word of an instruction, and whether it was written between quotes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Word {
    /// The word without its quotes.
    pub text: String,
    /// Whether any part of the word was quoted (a quoted word is never an
    /// instruction name or keyword).
    pub quoted: bool,
}

/// Quote state of a scan.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quote {
    None,
    Single,
    Double,
}

/// Splits `line` at every `;` outside quotes. Each instruction keeps its `;`,
/// and a last instruction written without one gets it. The game accepts one
/// instruction per line; this finds out how many a line holds.
pub fn split_instructions(line: &str) -> Vec<String> {
    let mut instructions = Vec::new();
    let mut current = String::new();
    let mut quote = Quote::None;
    let mut escaped = false;
    for c in line.chars() {
        current.push(c);
        if escaped {
            escaped = false;
            continue;
        }
        match (quote, c) {
            (Quote::Single, '\'') | (Quote::Double, '"') => quote = Quote::None,
            (Quote::Single, _) => {}
            (Quote::Double, '\\') | (Quote::None, '\\') => escaped = true,
            (Quote::Double, _) => {}
            (Quote::None, '\'') => quote = Quote::Single,
            (Quote::None, '"') => quote = Quote::Double,
            (Quote::None, ';') => {
                let instruction = current.trim();
                if instruction != ";" {
                    instructions.push(instruction.to_owned());
                }
                current.clear();
            }
            (Quote::None, _) => {}
        }
    }
    let rest = current.trim();
    if !rest.is_empty() {
        instructions.push(format!("{rest};"));
    }
    instructions
}

/// Splits one instruction (with or without its `;`) into words. As in the
/// text syntax, spaces outside quotes separate words, except inside an
/// expression (`[[ ... ]]`) closed later on the line.
pub fn split_words(instruction: &str) -> Vec<Word> {
    let body = instruction.trim();
    let body = body.strip_suffix(';').unwrap_or(body);
    let mut words = Vec::new();
    let mut word: Option<Word> = None;
    let mut quote = Quote::None;
    let mut depth = 0usize;
    let mut chars = body.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if quote == Quote::None {
            let pair = chars.peek().map(|(_, next)| *next) == Some(c);
            if pair && c == '[' {
                if depth > 0 || body[at + 2..].contains("]]") {
                    depth += 1;
                }
                let current = word.get_or_insert_with(|| Word {
                    text: String::new(),
                    quoted: false,
                });
                current.text.push(c);
                current.text.push(c);
                chars.next();
                continue;
            }
            if pair && c == ']' && depth > 0 {
                depth -= 1;
            }
        }
        match (quote, c) {
            (Quote::None, c) if c.is_whitespace() && depth == 0 => {
                if let Some(done) = word.take() {
                    words.push(done);
                }
                continue;
            }
            _ => {}
        }
        let current = word.get_or_insert_with(|| Word {
            text: String::new(),
            quoted: false,
        });
        match (quote, c) {
            (Quote::None, '\'') => {
                quote = Quote::Single;
                current.quoted = true;
            }
            (Quote::None, '"') => {
                quote = Quote::Double;
                current.quoted = true;
            }
            (Quote::Single, '\'') | (Quote::Double, '"') => quote = Quote::None,
            (Quote::None, '\\') => {
                if let Some((_, next)) = chars.next() {
                    current.text.push(next);
                }
            }
            (Quote::Double, '\\') => {
                current.text.push('\\');
                if let Some((_, next)) = chars.next() {
                    current.text.push(next);
                }
            }
            (_, c) => current.text.push(c),
        }
    }
    if let Some(done) = word {
        words.push(done);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(words: &[Word]) -> Vec<&str> {
        words.iter().map(|w| w.text.as_str()).collect()
    }

    #[test]
    fn splits_instructions_outside_quotes() {
        assert_eq!(split_instructions("tbs; tua;"), ["tbs;", "tua;"]);
        assert_eq!(split_instructions("tbs"), ["tbs;"]);
        assert_eq!(
            split_instructions("ate \"a;b\"; tua"),
            ["ate \"a;b\";", "tua;"]
        );
        assert_eq!(split_instructions("ate 'x;'"), ["ate 'x;';"]);
        assert_eq!(split_instructions("ate \"\\\";\""), ["ate \"\\\";\";"]);
        assert!(split_instructions("   ").is_empty());
        assert!(split_instructions(";;").is_empty());
    }

    #[test]
    fn splits_words_and_marks_quotes() {
        let words = split_words("ate \" fruit!\";");
        assert_eq!(texts(&words), ["ate", " fruit!"]);
        assert!(!words[0].quoted && words[1].quoted);
        assert_eq!(
            texts(&split_words("iter 3 times dlf")),
            ["iter", "3", "times", "dlf"]
        );
        assert_eq!(texts(&split_words("raw 'é' \"🙂\";")), ["raw", "é", "🙂"]);
        assert_eq!(texts(&split_words("ate \"\";")), ["ate", ""]);
        assert_eq!(
            texts(&split_words("onln [[__COUNTER__ + 1]] apply tua")),
            ["onln", "[[__COUNTER__ + 1]]", "apply", "tua"]
        );
        assert_eq!(
            texts(&split_words("ate {{ a }};")),
            ["ate", "{{", "a", "}}"]
        );
        assert_eq!(texts(&split_words("ate [[ a")), ["ate", "[[", "a"]);
        assert!(split_words(";").is_empty());
    }
}
