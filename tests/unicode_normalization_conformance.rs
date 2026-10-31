//! Offline Unicode conformance corpus, tied to the dependency's table version.
use std::{borrow::Cow, collections::HashSet};
use textforge::tokens::{InstructionMethods, transforms::unorm::Unorm};

const CORPUS: &str = include_str!("data/unicode/18.0.0/NormalizationTest.txt");

#[test]
fn official_normalization_corpus_and_idempotence() {
    assert_eq!(
        unicode_normalization::UNICODE_VERSION,
        (18, 0, 0),
        "Refresh corpus when Unicode tables change"
    );
    assert!(CORPUS.starts_with("# NormalizationTest-18.0.0.txt"));
    let tokens = ["nfc", "nfd", "nfkc", "nfkd"].map(|mode| Unorm::new(mode).unwrap());
    let mut count = 0;
    for (line_number, line) in CORPUS.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() || line.starts_with('@') {
            continue;
        }
        let columns: Vec<String> = line
            .split(';')
            .take(5)
            .map(|column| {
                column
                    .split_whitespace()
                    .map(|hex| char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap())
                    .collect()
            })
            .collect();
        assert_eq!(columns.len(), 5);
        for (mode, token) in tokens.iter().enumerate() {
            for (column, input) in columns.iter().enumerate() {
                let expected = match mode {
                    0 => {
                        if column < 3 {
                            &columns[1]
                        } else {
                            &columns[3]
                        }
                    }
                    1 => {
                        if column < 3 {
                            &columns[2]
                        } else {
                            &columns[4]
                        }
                    }
                    2 => &columns[3],
                    _ => &columns[4],
                };
                let result = token.transform(Cow::Borrowed(input), None).unwrap();
                assert_eq!(
                    &*result,
                    expected,
                    "line {}, mode {}, column {}",
                    line_number + 1,
                    mode,
                    column
                );
                assert_eq!(token.transform(result.clone(), None).unwrap(), result);
            }
        }
        count += 1;
    }
    assert!(count > 20_000, "Corpus must not be truncated");
    eprintln!(
        "Verified {count} official Unicode 18 normalization rows (20 transformations per row)"
    );
}

#[test]
fn unlisted_scalars_normalize_to_themselves() {
    let mut part_one = false;
    let mut listed = HashSet::new();
    for line in CORPUS.lines() {
        if line.starts_with("@Part") {
            part_one = line.starts_with("@Part1 ");
        }
        if !part_one {
            continue;
        }
        let content = line.split('#').next().unwrap().trim();
        if content.is_empty() || content.starts_with('@') {
            continue;
        }
        let first = content.split(';').next().unwrap().trim();
        assert_eq!(first.split_whitespace().count(), 1);
        listed.insert(u32::from_str_radix(first, 16).unwrap());
    }
    assert!(listed.len() > 10_000);
    let tokens = ["nfc", "nfd", "nfkc", "nfkd"].map(|mode| Unorm::new(mode).unwrap());
    for code in 0..=0x10ffff {
        if listed.contains(&code) {
            continue;
        }
        if let Some(character) = char::from_u32(code) {
            let mut bytes = [0; 4];
            let text: &str = character.encode_utf8(&mut bytes);
            for token in &tokens {
                assert_eq!(
                    token.transform(Cow::Borrowed(text), None).unwrap(),
                    text,
                    "U+{code:04X}"
                );
            }
        }
    }
}
