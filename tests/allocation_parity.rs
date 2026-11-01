//! Exact outputs and structured errors captured from core revision 72fc7fc.
#![cfg(feature = "test_access")]
#[path = "../examples/support/allocation_workload.rs"]
mod workload;

#[test]
fn all_instructions_match_the_original_for_owned_and_borrowed_inputs() {
    let fixtures: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("data/allocations/baseline.json")).unwrap();
    for fixture in fixtures {
        let case = &fixture["case"];
        let input = fixture["input"].as_str().unwrap();
        let owned = fixture["owned"].as_bool().unwrap();
        let result = std::panic::catch_unwind(|| workload::execute(case, input, owned));
        let outcome = match result {
            Ok(Ok(text)) => serde_json::json!({"ok":text}),
            Ok(Err(error)) => serde_json::json!({"error":format!("{:?}",error)}),
            Err(_) => serde_json::json!({"panic":true}),
        };
        assert_eq!(
            &outcome,
            if cfg!(debug_assertions) {
                fixture.get("outcome_debug").unwrap_or(&fixture["outcome"])
            } else {
                &fixture["outcome"]
            },
            "{} input={input:?}, owned={owned}",
            case["name"]
        );
    }
}

#[test]
fn benchmark_covers_every_registered_instruction_and_each_case_executes() {
    let cases = workload::cases().unwrap();
    let mut names: Vec<_> = cases.iter().map(|c| c["name"].as_str().unwrap()).collect();
    names.sort_unstable();
    let source = include_str!("../src/globals/table.rs");
    let pattern = regex::Regex::new(r#"\(\s*"([a-z]+)",\s*0x"#).unwrap();
    let mut registered: Vec<_> = pattern
        .captures_iter(source)
        .map(|c| c.get(1).unwrap().as_str())
        .collect();
    registered.sort_unstable();
    assert_eq!(names, registered);
    let input = "banana banana banana banana banana banana banana banana\r\nsecond\r\nthird\r\n";
    let mut processor = textforge::api::processor::TextForgeProcessor::new();
    for case in &cases {
        workload::lines(case).unwrap();
        workload::register(&mut processor, case).unwrap();
        workload::execute(case, input, false).unwrap();
    }
}
