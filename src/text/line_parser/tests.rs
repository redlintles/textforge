use super::{quote, split_line};
use crate::utils::errors::TextForgeErrorCode;

fn words(line: &str) -> Vec<String> {
    split_line(line).unwrap()
}

fn assert_parse_error(line: &str) {
    let error = split_line(line).unwrap_err();
    assert!(
        matches!(error.error_code, TextForgeErrorCode::TextParsingError(_)),
        "{line:?}: {error:?}"
    );
}

/// True when `line` uses none of the rules that changed from shell_words:
/// control escapes (`\n`, `\t`, `\r`, `\0` after a backslash) and real line breaks.
fn uses_only_legacy_rules(line: &str) -> bool {
    if line.contains(['\n', '\r']) {
        return false;
    }
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' {
            if matches!(bytes.get(i + 1), Some(b'n' | b't' | b'r' | b'0')) {
                return false;
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    true
}

/// Small deterministic generator, so the differential test needs no extra crate.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }
}

fn random_line(rng: &mut Lcg, alphabet: &[&str]) -> String {
    let len = rng.next() % 16;
    (0..len)
        .map(|_| alphabet[(rng.next() as usize) % alphabet.len()])
        .collect()
}

const ALPHABET: &[&str] = &[
    "a",
    "b",
    " ",
    "\t",
    "'",
    "\"",
    "\\",
    "#",
    "$",
    "`",
    ";",
    "n",
    "t",
    "r",
    "0",
    "d",
    "é",
    "🦀",
    "e\u{301}",
    "{{x}}",
    "[[ 1 + 2 ]]",
    "\n",
];

#[test]
fn matches_shell_words_on_every_pipeline_in_the_repository() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut pending = vec![root.to_path_buf()];
    let mut checked = 0;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    pending.push(path);
                }
                continue;
            }
            if !(name.ends_with(".textforge") || name.ends_with(".tft")) {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            for line in content.lines() {
                let line = line.trim();
                let Some(body) = line.strip_suffix(';') else {
                    continue;
                };
                if !uses_only_legacy_rules(body) {
                    continue;
                }
                assert_eq!(
                    split_line(body).ok(),
                    shell_words::split(body).ok(),
                    "{}: {line}",
                    path.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 0, "no pipeline lines found");
}

#[test]
fn matches_shell_words_on_random_lines_without_new_escapes() {
    let mut rng = Lcg(0x7e47_f0e9);
    let mut compared = 0;
    for _ in 0..200_000 {
        let line = random_line(&mut rng, ALPHABET);
        if !uses_only_legacy_rules(&line) {
            continue;
        }
        assert_eq!(
            split_line(&line).ok(),
            shell_words::split(&line).ok(),
            "{line:?}"
        );
        compared += 1;
    }
    assert!(compared > 50_000);
}

#[test]
fn quote_matches_shell_words_without_line_breaks_and_round_trips() {
    let mut rng = Lcg(0x5eed);
    for _ in 0..200_000 {
        let value = random_line(&mut rng, ALPHABET);
        if !value.contains(['\n', '\r']) {
            assert_eq!(quote(&value), shell_words::quote(&value), "{value:?}");
        }
        assert_eq!(words(&quote(&value)), vec![value.clone()], "{value:?}");
    }
}

#[test]
fn double_quotes_resolve_control_escapes() {
    assert_eq!(
        words(r#"atb "banana\nlaranja""#),
        vec!["atb", "banana\nlaranja"]
    );
    assert_eq!(words(r#"raw " " "\t""#), vec!["raw", " ", "\t"]);
    assert_eq!(words(r#"atb "a\rb\0c""#), vec!["atb", "a\rb\0c"]);
    assert_eq!(words(r#"atb "\\n""#), vec!["atb", "\\n"]);
    assert_eq!(words(r#"atb "\"\$\`""#), vec!["atb", "\"$`"]);
}

#[test]
fn other_escapes_in_double_quotes_keep_the_backslash() {
    // Regex patterns depend on this: "\d" must reach the regex as \d.
    assert_eq!(words(r#"klm "^\d+\s\w$""#), vec!["klm", r"^\d+\s\w$"]);
    assert_eq!(words(r#"atb "\é\🦀""#), vec!["atb", "\\é\\🦀"]);
}

#[test]
fn single_quotes_stay_literal() {
    assert_eq!(words(r"atb 'a\nb\t'"), vec!["atb", r"a\nb\t"]);
    assert_eq!(words("atb 'it'\\''s'"), vec!["atb", "it's"]);
}

#[test]
fn control_escapes_outside_double_quotes_are_errors() {
    for line in [r"atb banana\nlaranja", r"atb \t", r"atb a\r", r"atb \0"] {
        assert_parse_error(line);
    }
    // Other unquoted escapes keep their previous meaning.
    assert_eq!(words(r"atb a\ b\d"), vec!["atb", "a bd"]);
    assert_eq!(words(r"atb a\"), vec!["atb", "a\\"]);
}

#[test]
fn real_line_breaks_are_errors_everywhere() {
    for line in [
        "atb a\nb",
        "atb \"a\nb\"",
        "atb 'a\nb'",
        "atb \"a\\\nb\"",
        "atb a\\\nb",
        "atb a\rb",
    ] {
        assert_parse_error(line);
    }
}

#[test]
fn unterminated_quotes_are_errors() {
    for line in [r#"atb "abc"#, "atb 'abc", r#"atb "abc\"#] {
        assert_parse_error(line);
    }
}

#[test]
fn empty_words_comments_and_unicode() {
    assert_eq!(words(r#"atb """#), vec!["atb", ""]);
    assert_eq!(words("atb ''"), vec!["atb", ""]);
    assert_eq!(words("atb a #comment"), vec!["atb", "a"]);
    assert_eq!(words("atb a#b"), vec!["atb", "a#b"]);
    assert_eq!(words("atb '#b'"), vec!["atb", "#b"]);
    assert_eq!(
        words("atb   é\t🦀  e\u{301}"),
        vec!["atb", "é", "🦀", "e\u{301}"]
    );
    assert_eq!(words(r#"atb "é\n🦀""#), vec!["atb", "é\n🦀"]);
    assert!(words("").is_empty());
    assert!(words("   ").is_empty());
}

#[test]
fn errors_carry_no_copy_of_the_line() {
    // read_from_text names the line in `instruction`; the lexer adds nothing.
    let long = format!("atb \"{}", "x".repeat(10_000));
    let error = split_line(&long).unwrap_err();
    assert!(error.input.is_empty());
    assert!(error.instruction.is_empty());
}

/// Parse time of `split_line` vs `shell_words::split` over every pipeline line
/// in the repository. Run with `scripts/benchmark_line_parser.py`.
#[test]
#[ignore = "benchmark; run with scripts/benchmark_line_parser.py"]
fn benchmark_against_shell_words() {
    use std::time::Instant;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut lines = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            if path.is_dir() {
                if !name.starts_with('.') && name != "target" {
                    pending.push(path);
                }
            } else if name.ends_with(".textforge") || name.ends_with(".tft") {
                for line in std::fs::read_to_string(&path).unwrap().lines() {
                    if let Some(body) = line.trim().strip_suffix(';')
                        && uses_only_legacy_rules(body)
                    {
                        lines.push(body.to_owned());
                    }
                }
            }
        }
    }
    const ROUNDS: usize = 2_000;
    let measure = |f: &dyn Fn(&str) -> usize| {
        let mut best = f64::MAX;
        for _ in 0..5 {
            let start = Instant::now();
            let mut sink = 0;
            for _ in 0..ROUNDS {
                for line in &lines {
                    sink += f(std::hint::black_box(line));
                }
            }
            std::hint::black_box(sink);
            best = best.min(start.elapsed().as_secs_f64());
        }
        best * 1e9 / (ROUNDS * lines.len()) as f64
    };
    let ours = measure(&|l| split_line(l).map_or(0, |w| w.len()));
    let reference = measure(&|l| shell_words::split(l).map_or(0, |w| w.len()));
    println!(
        "lines={} split_line={ours:.1}ns/line shell_words={reference:.1}ns/line ratio={:.2}",
        lines.len(),
        reference / ours
    );
}
