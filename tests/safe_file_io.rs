//! Whole-document I/O regressions, exercised with every feature configuration.
use std::{fs, path::PathBuf};
use textforge::{
    api::{
        TextForgeBuilderMethods,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    utils::errors::TextForgeErrorCode,
};

fn directory() -> PathBuf {
    let path = std::env::temp_dir().join(format!("textforge-safe-io-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn file_preserves_unicode_newlines_nul_empty_and_large_documents() {
    let directory = directory();
    let mut processor = TextForgeProcessor::new();
    let id = processor
        .create_pipeline()
        .add_to_beginning("<")
        .unwrap()
        .add_to_end(">")
        .unwrap()
        .build();
    for text in [String::new(), "João é 👩🏽‍💻 🇧🇷\r\n\0".repeat(5000)] {
        let input = directory.join("input");
        let output = directory.join("nested/output");
        fs::write(&input, &text).unwrap();
        processor.process_file(&id, &input, &output).unwrap();
        assert_eq!(fs::read_to_string(output).unwrap(), format!("<{text}>"));
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_reads_before_replacing_the_same_path() {
    let directory = directory();
    let file = directory.join("document");
    fs::write(&file, "é 👩🏽‍💻").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.create_pipeline().add_to_end("!").unwrap().build();
    processor.process_file(&id, &file, &file).unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "é 👩🏽‍💻!");
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_errors_keep_existing_output_untouched() {
    let directory = directory();
    let input = directory.join("input");
    let output = directory.join("output");
    fs::write(&output, "previous").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.create_pipeline().add_to_end("!").unwrap().build();
    let missing = processor.process_file(&id, &input, &output).unwrap_err();
    assert!(matches!(
        missing.error_code,
        TextForgeErrorCode::FileOpeningError(_)
    ));
    fs::write(&input, [b'a', 0xff]).unwrap();
    let invalid = processor.process_file(&id, &input, &output).unwrap_err();
    assert!(matches!(
        invalid.error_code,
        TextForgeErrorCode::InvalidUtf8Error(_)
    ));
    fs::write(&input, "valid").unwrap();
    assert!(processor.process_file("unknown", &input, &output).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "previous");
    assert!(processor.process_file(&id, &directory, &output).is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn output_path_errors_are_propagated() {
    let directory = directory();
    let input = directory.join("input");
    let blocker = directory.join("file-instead-of-directory");
    fs::write(&input, "valid").unwrap();
    fs::write(&blocker, "blocking").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = processor.create_pipeline().add_to_end("!").unwrap().build();
    assert!(
        processor
            .process_file(&id, &input, &blocker.join("output"))
            .is_err()
    );
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(1)
        .build()
        .unwrap();
    let blocked_output = blocker.join("output");
    let results = processor.process_batch_in_pool(&pool, vec![(&input, &id, &blocked_output)]);
    assert!(results[0].is_err());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn reusable_pools_keep_batch_order_partial_errors_and_document_isolation() {
    let directory = directory();
    let a = directory.join("a");
    let b = directory.join("b");
    let empty = directory.join("empty");
    let invalid = directory.join("invalid");
    let missing = directory.join("missing");
    fs::write(&a, "João\r\n").unwrap();
    fs::write(&b, "é 👩🏽‍💻").unwrap();
    fs::write(&empty, "").unwrap();
    fs::write(&invalid, [0xff]).unwrap();
    let mut processor = TextForgeProcessor::new();
    let pipeline = directory.join("pipeline.tft");
    fs::write(&pipeline, "var suffix = \"!\";\nate \"{{suffix}}\";\n").unwrap();
    let id = processor.get_pipeline_from_file(&pipeline).unwrap();
    for threads in [1, 4] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        assert!(processor.process_batch_in_pool(&pool, vec![]).is_empty());
        for _ in 0..2 {
            let outputs: Vec<_> = (0..7)
                .map(|i| directory.join(format!("nested/{i}")))
                .collect();
            let tasks = vec![
                (a.as_path(), id.as_str(), outputs[0].as_path()),
                (missing.as_path(), id.as_str(), outputs[1].as_path()),
                (b.as_path(), id.as_str(), outputs[2].as_path()),
                (invalid.as_path(), id.as_str(), outputs[3].as_path()),
                (empty.as_path(), id.as_str(), outputs[4].as_path()),
                (directory.as_path(), id.as_str(), outputs[5].as_path()),
                (b.as_path(), "unknown", outputs[6].as_path()),
            ];
            let results = processor.process_batch_in_pool(&pool, tasks);
            assert_eq!(
                results.iter().map(Result::is_ok).collect::<Vec<_>>(),
                [true, false, true, false, true, false, false]
            );
            assert!(matches!(
                results[1].as_ref().unwrap_err().error_code,
                TextForgeErrorCode::FileNotFound(_)
            ));
            assert!(matches!(
                results[3].as_ref().unwrap_err().error_code,
                TextForgeErrorCode::InvalidUtf8Error(_)
            ));
            assert_eq!(fs::read_to_string(&outputs[0]).unwrap(), "João\r\n!");
            assert_eq!(fs::read_to_string(&outputs[2]).unwrap(), "é 👩🏽‍💻!");
            assert_eq!(fs::read_to_string(&outputs[4]).unwrap(), "!");
            for i in [1, 3, 5, 6] {
                assert!(!outputs[i].exists());
            }
        }
    }
    fs::remove_dir_all(directory).unwrap();
}
