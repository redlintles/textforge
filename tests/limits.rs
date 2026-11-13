//! Default limits stop instructions that multiply work before they exhaust
//! memory, disk or CPU; profiles raise or remove them (`docs/execution-profiles.md`).
#![cfg(feature = "test_access")]

use std::time::Instant;

use textforge::{
    api::{
        ExecutionProfile, Limits, TextBuffer, TextBufferOptions,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    text::reader::read_from_text,
    utils::errors::TextForgeErrorCode,
};

fn register(processor: &mut TextForgeProcessor, lines: &[&str]) -> String {
    processor.add_transform(lines.iter().map(|l| read_from_text(l).unwrap()).collect())
}

fn limited(generated: u64, iterations: u64) -> ExecutionProfile {
    ExecutionProfile {
        limits: Limits {
            max_generated_bytes: Some(generated),
            max_iterations: Some(iterations),
        },
        ..ExecutionProfile::balanced()
    }
}

fn is_limit(error: &textforge::utils::errors::TextForgeError) -> bool {
    matches!(error.error_code, TextForgeErrorCode::ValidationError(_))
        && error.input.contains(".txfrc")
}

#[test]
fn a_huge_repetition_fails_at_once_even_when_spooled_to_disk() {
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor, &["rpt 18446744073709551615;"]);
    let start = Instant::now();
    assert!(is_limit(&processor.process_all(&id, "a").unwrap_err()));
    // Spooled: previously wrote blocks until the disk was full.
    let directory = tempfile::tempdir().unwrap();
    let buffer = TextBuffer::from_text("a", TextBufferOptions::new(1, directory.path())).unwrap();
    assert!(is_limit(
        &processor.process_all_buffered(&id, buffer).unwrap_err()
    ));
    assert!(start.elapsed().as_secs() < 5);
}

#[test]
fn each_multiplying_instruction_respects_its_limit() {
    let mut processor = TextForgeProcessor::new().with_profile(limited(10, 3));
    for (line, fits, exceeds) in [
        ("rpt 5;", "ab", "abc"), // 10 bytes fit; 15 do not
        ("padl x 10;", "", "é"), // 10 bytes fit; "é" + 9 "x" = 11 do not
        ("padr é 5;", "", "a"),  // 5 × "é" = 10 bytes fit; "a" + 4 × "é" = 9 fit too
    ] {
        let id = register(&mut processor, &[line]);
        assert!(processor.process_all(&id, fits).is_ok(), "{line} {fits:?}");
        let result = processor.process_all(&id, exceeds);
        if line.starts_with("padr") {
            assert!(result.is_ok());
        } else {
            assert!(is_limit(&result.unwrap_err()), "{line}");
        }
    }
    // 6 × "é" = 12 bytes.
    let id = register(&mut processor, &["padr é 6;"]);
    assert!(is_limit(&processor.process_all(&id, "").unwrap_err()));
    let id = register(&mut processor, &["iter 3 times ate x;"]);
    assert_eq!(processor.process_all(&id, "").unwrap(), "xxx");
    let id = register(&mut processor, &["iter 4 times ate x;"]);
    assert!(is_limit(&processor.process_all(&id, "").unwrap_err()));
}

#[test]
fn limits_reach_nested_and_joined_instructions() {
    let directory = tempfile::tempdir().unwrap();
    let library = directory.path().join("lib.textforge");
    std::fs::write(&library, "rpt 4;\n").unwrap();
    let main = directory.path().join("main.textforge");
    std::fs::write(&main, "ppjn lib.textforge;\n").unwrap();
    let mut processor = TextForgeProcessor::new().with_profile(limited(8, 3));
    let joined = processor.get_pipeline_from_file(&main).unwrap();
    assert_eq!(processor.process_all(&joined, "ab").unwrap(), "abababab");
    assert!(is_limit(
        &processor.process_all(&joined, "abc").unwrap_err()
    ));

    let nested = register(&mut processor, &["onln 2 apply rpt 9;"]);
    assert!(is_limit(
        &processor.process_all(&nested, "x\nyy").unwrap_err()
    ));
    let id = register(
        &mut processor,
        &["blk b assoc iter 4 times ate x;", "cblk b;"],
    );
    assert!(is_limit(&processor.process_all(&id, "").unwrap_err()));
    let single = read_from_text("rpt 9;").unwrap();
    assert!(is_limit(
        &processor.process_single(single, "ab").unwrap_err()
    ));
}

#[test]
fn raised_or_removed_limits_change_nothing_but_the_failures() {
    let mut processor = TextForgeProcessor::new().with_profile(limited(4, 1));
    let id = register(&mut processor, &["rpt 3;", "iter 2 times ate '!';"]);
    assert!(processor.process_all(&id, "ab").is_err());
    processor.set_profile(ExecutionProfile {
        limits: Limits::unlimited(),
        ..ExecutionProfile::balanced()
    });
    let unlimited = processor.process_all(&id, "ab").unwrap();
    processor.set_profile(ExecutionProfile::default());
    assert_eq!(processor.process_all(&id, "ab").unwrap(), unlimited);
    assert_eq!(unlimited, "ababab!!");
}
