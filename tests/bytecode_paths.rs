//! Both bytecode serializers write the same record: `to_bytecode()` (used by
//! `TokenWrapper::to_bytecode_resolved` and nested resolved parameters) and the
//! generic `to_bytecode_unresolved` used for files. `eval` and `rmp` used to
//! write zero parameters from `to_bytecode()`.
#![cfg(feature = "test_access")]

use textforge::{context::execution_context::GlobalExecutionContext, text::reader::read_from_text};

#[test]
fn every_instruction_serializes_the_same_way_on_both_paths() {
    let cases: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/allocations/cases.json")).unwrap();
    let mut checked = 0;
    for case in &cases {
        for line in case["lines"].as_array().unwrap() {
            let line = line.as_str().unwrap();
            let token = read_from_text(line).unwrap();
            // Lines whose parameters need runtime values cannot resolve here.
            let Ok(resolved) = token.to_bytecode_resolved(&mut GlobalExecutionContext::new())
            else {
                continue;
            };
            let unresolved = token.to_bytecode_unresolved().unwrap();
            assert_eq!(resolved, unresolved, "{line}");
            checked += 1;
        }
    }
    assert!(checked > 80, "only {checked} lines checked");
}

#[test]
fn eval_and_rmp_records_carry_their_parameters() {
    for line in ["eval '1 + 2' in total;", "rmp '[0-9]+';"] {
        let token = read_from_text(line).unwrap();
        let record = token
            .to_bytecode_resolved(&mut GlobalExecutionContext::new())
            .unwrap();
        // size (8) + opcode (4) + parameter count (1)
        let expected = if line.starts_with("eval") { 2 } else { 1 };
        assert_eq!(record[12], expected, "{line}");
    }
}
