//! The instruction index (rustdoc of `tokens`, module lists and README) lists
//! every registered instruction exactly once. Regenerate it with
//! `python3 scripts/instruction_index.py`.

use std::collections::BTreeSet;

fn registered() -> BTreeSet<String> {
    let source = include_str!("../src/globals/table.rs");
    let pattern = regex::Regex::new(r#"\(\s*"([a-z]+)",\s*0x"#).unwrap();
    pattern
        .captures_iter(source)
        .map(|c| c[1].to_owned())
        .collect()
}

fn listed(text: &str, row: &str) -> Vec<String> {
    let pattern = regex::Regex::new(row).unwrap();
    pattern
        .captures_iter(text)
        .map(|c| c[1].to_owned())
        .collect()
}

fn assert_lists_each_once(name: &str, mut found: Vec<String>) {
    found.sort();
    let unique: BTreeSet<_> = found.iter().cloned().collect();
    assert_eq!(
        found.len(),
        unique.len(),
        "{name} lists an instruction twice"
    );
    assert_eq!(unique, registered(), "{name} is out of date");
}

#[test]
fn rustdoc_index_lists_every_instruction() {
    let text = include_str!("../src/tokens/mod.rs");
    assert_lists_each_once(
        "src/tokens/mod.rs",
        listed(text, r"(?m)^//! \| \[`([a-z]+)`\]"),
    );
}

#[test]
fn readme_index_lists_every_instruction() {
    let text = include_str!("../README.md");
    assert_lists_each_once("README.md", listed(text, r"(?m)^\| `([a-z]+)` \| `"));
}

#[test]
fn every_instruction_module_is_described() {
    let mut found = listed(
        include_str!("../src/tokens/transforms/mod.rs"),
        r"(?m)^/// `([a-z]+)[ ;]",
    );
    found.extend(listed(
        include_str!("../src/tokens/instructions/mod.rs"),
        r"(?m)^/// `([a-z]+)[ ;]",
    ));
    assert_lists_each_once("module lists", found);
}
