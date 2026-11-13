use super::*;
use crate::api::processor::TextForgeProcessorMethods;
use std::borrow::Cow;

fn pipeline(processor: &mut TextForgeProcessor, source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.tft");
    std::fs::write(&path, source).unwrap();
    processor.get_pipeline_from_file(&path).unwrap()
}

#[test]
fn adapters_preserve_legacy_transforms_context_and_halt() {
    for source in [
        "atb <;\nate >;\n",
        "raw alice USER;\ntua;\n",
        "atb A;\nend;\nate B;\n",
        "blk x assoc tua;\ncblk x;\n",
        "var name = João;\natb \"{{name}}: \";\n",
    ] {
        for input in [
            String::new(),
            "alice é 👩🏽‍💻 🇧🇷".into(),
            "alice é 🦀".repeat(180000),
        ] {
            let mut processor = TextForgeProcessor::new();
            let id = pipeline(&mut processor, source);
            let expected = processor
                .run_transform(&id, Cow::Borrowed(&input))
                .unwrap()
                .into_owned();
            assert_eq!(processor.process_all(&id, &input).unwrap(), expected);
            let mut log = Vec::new();
            let document =
                TextBuffer::from_text(&input, TextBufferOptions::new(1, std::env::temp_dir()))
                    .unwrap();
            assert_eq!(
                processor
                    .run_buffered_debug(&id, document, &mut log)
                    .unwrap()
                    .into_string()
                    .unwrap(),
                expected
            );
            assert!(log.starts_with(b"PROCESSING STEP BY STEP:\n----------\n\n"));
        }
    }
}

#[test]
fn batch_keeps_order_and_continues_after_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb <;\nate >;\n");
    let input = dir.path().join("input");
    let bad = dir.path().join("bad");
    let missing = dir.path().join("missing");
    let output = dir.path().join("nested/output");
    let output2 = dir.path().join("output2");
    std::fs::write(&input, "é 👩🏽‍💻".repeat(100000)).unwrap();
    std::fs::write(&bad, [0xff]).unwrap();
    let results = processor.process_batch(vec![
        (&bad, &id, &output2),
        (&input, &id, &output),
        (&missing, &id, &output2),
    ]);
    assert!(results[0].is_err());
    assert!(results[1].is_ok());
    assert!(results[2].is_err());
    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        format!("<{}>", std::fs::read_to_string(input).unwrap())
    );
    assert!(processor.process_batch(vec![]).is_empty());
}

#[test]
fn adapters_record_pipeline_and_parameter_errors() {
    let mut processor = TextForgeProcessor::new();
    assert!(processor.process_all("missing", "").is_err());
    assert!(processor.process_all_with_debug("missing", "").is_err());
    let id = pipeline(&mut processor, "atb \"{{missing}}\";\n");
    assert!(processor.process_all(&id, "abc").is_err());
    assert!(processor.process_all_with_debug(&id, "abc").is_err());
}

#[test]
#[cfg(feature = "watchers")]
fn watchers_preserve_snapshots_lookahead_and_reset() {
    use crate::watchers::{ExecutionWindows, WatcherContext, WatcherList};
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb <;\nate >;\n");
    let mut list = WatcherList::new();
    list.set_watcher("snapshot", |ctx: WatcherContext| {
        serde_json::json!({
            "before": ctx.before.as_ref(), "current": ctx.current.as_ref(),
            "after": ctx.after.as_deref(), "instruction": ctx.instruction.as_ref()
        })
        .to_string()
    });
    list.schedule_watcher("snapshot", ExecutionWindows::All)
        .unwrap();
    for input in [String::new(), "é 👩🏽‍💻 🇧🇷".into(), "é 🦀".repeat(250000)] {
        let report = directory.path().join("report.json");
        assert_eq!(
            processor
                .process_all_with_watchers(&id, &input, &mut list, &report)
                .unwrap(),
            format!("<{input}>")
        );
        let rows: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        let first: serde_json::Value =
            serde_json::from_str(rows[0]["snapshot"].as_str().unwrap()).unwrap();
        let last: serde_json::Value =
            serde_json::from_str(rows[1]["snapshot"].as_str().unwrap()).unwrap();
        assert_eq!(first["before"], input);
        assert_eq!(first["current"], format!("<{input}"));
        assert_eq!(first["after"], format!("<{input}>"));
        assert_eq!(last["before"], format!("<{input}"));
        assert_eq!(last["current"], format!("<{input}>"));
        assert!(last["after"].is_null());
    }
}

#[test]
#[cfg(feature = "watchers")]
fn watchers_propagate_errors_and_retry_export_without_resetting_results() {
    use crate::watchers::{ExecutionWindows, WatcherContext, WatcherList};
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let mut list = WatcherList::new();
    list.set_watcher("bytes", |ctx: WatcherContext| ctx.current.len().to_string());
    list.schedule_watcher("bytes", ExecutionWindows::All)
        .unwrap();
    let report = directory.path().join("report.json");
    assert!(
        processor
            .process_all_with_watchers("missing", "abc", &mut list, &report)
            .is_err()
    );
    assert!(!report.exists());
    let invalid = pipeline(&mut processor, "atb \"{{missing}}\";\n");
    assert!(
        processor
            .process_all_with_watchers(&invalid, "abc", &mut list, &report)
            .is_err()
    );
    assert!(!report.exists());
    list.reset();
    let valid = pipeline(&mut processor, "atb <;\nate >;\n");
    let invalid_report = directory.path().join("missing/report.json");
    assert!(
        processor
            .process_all_with_watchers(&valid, "abc", &mut list, &invalid_report)
            .is_err()
    );
    list.to_json(&report).unwrap();
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(rows[0]["bytes"], "4");
    assert_eq!(rows[1]["bytes"], "5");
    list.reset();
    assert_eq!(
        processor
            .process_all_with_watchers(&valid, "abc", &mut list, &report)
            .unwrap(),
        "<abc>"
    );
}

fn plain_debug_log(source: &str, input: &str) -> (String, String) {
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, source);
    let mut log = Vec::new();
    let document =
        TextBuffer::from_text(input, TextBufferOptions::new(1, std::env::temp_dir())).unwrap();
    let output = processor
        .run_buffered_debug(&id, document, &mut log)
        .unwrap()
        .into_string()
        .unwrap();
    let ansi = regex::Regex::new("\x1b\\[[0-9;]*m").unwrap();
    let log = ansi
        .replace_all(&String::from_utf8(log).unwrap(), "")
        .into_owned();
    (output, log)
}

use crate::context::debug_trace::fixtures::{NESTED_DEBUG_PIPELINE, NESTED_DEBUG_TRACE};

#[test]
fn debug_traces_blocks_maps_iterations_and_sections() {
    let (output, log) = plain_debug_log(NESTED_DEBUG_PIPELINE, "  cat   wolf  \nx");
    assert_eq!(output, "# WOLF  DOG \nx01");
    let mut rest = log.as_str();
    for expected in NESTED_DEBUG_TRACE {
        let at = rest
            .find(expected)
            .unwrap_or_else(|| panic!("missing {expected:?} in {log}"));
        rest = &rest[at + expected.len()..];
    }
    assert_eq!(log.matches('▶').count(), NESTED_DEBUG_TRACE.len());
}

#[test]
fn debug_trace_nests_containers_inside_blocks() {
    let source =
        "blk outer assoc iter 2 times onln 1 apply ate x;\nblk outer assoc dlf;\ncblk outer;\n";
    let (output, log) = plain_debug_log(source, "ab\ncd");
    assert_eq!(output, "bxx\ncd");
    let expected = "▶ cblk outer  (2 instructions)\n  ▶ iter 2 times\n    ▶ onln 1  (line: 2 chars)\n      │ 1. ate x  2 → 3 chars\n    ◀ onln  5 → 6 chars (+1) · 2 → 2 lines\n    ▶ onln 1  (line: 3 chars)\n      │ 1. ate x  3 → 4 chars\n    ◀ onln  6 → 7 chars (+1) · 2 → 2 lines\n  ◀ iter  5 → 7 chars (+2) · 2 → 2 lines\n  │ 2. dlf  7 → 6 chars\n◀ outer  5 → 6 chars (+1) · 2 → 2 lines\n";
    assert!(log.contains(expected), "{log}");
}

#[test]
fn debug_lists_at_most_twenty_replacements() {
    let source = "rmapd m add a b;\nrmape m;\n";
    let (output, log) = plain_debug_log(source, &"a".repeat(25));
    assert_eq!(output, "b".repeat(25));
    assert_eq!(log.matches("│ rule 1 /a/  'a' → 'b'").count(), 20);
    assert!(log.contains("│ rule 1 /a/  'a' → 'b'  @ 19..20\n  │ … +5 replacements\n"));
    assert!(log.contains("◀ m  25 → 25 chars (±0) · 1 → 1 line · 25 replacements"));
}

#[test]
fn debug_never_changes_results() {
    for source in [
        NESTED_DEBUG_PIPELINE,
        "blk outer assoc iter 2 times onln 1 apply ate x;\ncblk outer;\n",
        "rmapd m add 'é+' E;\nrmape m;\nonck 0 2 apply rev;\nonlni 1 2 apply uniqln;\n",
        "atb A;\nend;\nate B;\n",
    ] {
        for input in ["", "é é\né", "  cat   wolf  \nx\nx\n"] {
            let mut processor = TextForgeProcessor::new();
            let id = pipeline(&mut processor, source);
            let expected = processor.run_transform(&id, Cow::Borrowed(input));
            let (output, _) = match expected {
                Ok(_) => plain_debug_log(source, input),
                Err(_) => continue,
            };
            assert_eq!(output, expected.unwrap(), "{source} {input:?}");
            assert_eq!(
                processor.process_all_with_debug(&id, input).unwrap(),
                output
            );
        }
    }
}
