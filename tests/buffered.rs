#![cfg(feature = "buffered")]

use textforge::api::processor::{BufferedTask, TextForgeProcessor, TextForgeProcessorMethods};
use textforge::api::{TextBuffer, TextBufferOptions};

fn pipeline(processor: &mut TextForgeProcessor, source: &str) -> String {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pipeline.textforge");
    std::fs::write(&path, source).unwrap();
    processor.get_pipeline_from_file(&path).unwrap()
}

#[test]
fn preserves_legacy_results_for_native_and_fallback_instructions() {
    let cases = [
        ("null;\n", ""),
        ("atb HEADER;\n", ""),
        ("atb \"< \";\nate \" >\";\nrpt 3;\n", "👩🏽‍💻 é 🇧🇷"),
        ("dlf;\ndll;\nrev;\n", "x👨‍👩‍👧‍👦éx"),
        ("raw \"secret\" \"[MASKED]\";\n", "xxsecretxx"),
        ("tbs;\ntua;\ntla;\n", "  Straße ΟΣ café  "),
        (
            "urle;\nurld;\nhtmle;\nhtmlu;\njsone;\njsonu;\n",
            "<a>é 👩🏽‍💻\n",
        ),
        ("var name = João;\natb \"{{name}}: \";\n", "ok"),
        ("atb \"[[ 2 + 3 ]]\";\n", "value"),
        (
            "blk clean assoc tbs;\nblk clean assoc tla;\ncblk clean;\n",
            "  BANANA  ",
        ),
        ("ifdc x do atb PREFIX;\n", "xyz"),
        ("iter 3 times ate \"!\";\n", "ok"),
        ("atb A;\nend;\nate B;\n", "x"),
        ("null;\n", "a\r\nb\r\n\0"),
    ];
    for threshold in [0, 7, 1024 * 1024] {
        for (source, input) in cases {
            let mut processor = TextForgeProcessor::new();
            let id = pipeline(&mut processor, source);
            let expected = processor.process_all(&id, input).unwrap();
            let buffer = TextBuffer::from_text(
                input,
                TextBufferOptions::new(threshold, std::env::temp_dir()),
            )
            .unwrap();
            let actual = processor
                .process_all_buffered(&id, buffer)
                .unwrap()
                .into_string()
                .unwrap();
            assert_eq!(actual, expected, "{source:?}, threshold={threshold}");
        }
    }
}

#[test]
fn regex_matches_across_physical_read_boundaries() {
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "raw \"secret\" \"[MASKED]\";\n");
    let input = format!("{}secret{}", "x".repeat(65533), "y".repeat(65536));
    let expected = processor.process_all(&id, &input).unwrap();
    let buffer = TextBuffer::from_reader(
        input.as_bytes(),
        TextBufferOptions::new(8, std::env::temp_dir()),
    )
    .unwrap();
    assert_eq!(
        processor
            .process_all_buffered(&id, buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        expected
    );
}

#[test]
fn file_and_single_instruction_apis_preserve_document_and_newlines() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("input.txt");
    std::fs::write(&input, "👩🏽‍💻\r\né\r\n").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb \"<\";\nate \">\";\n");
    let result = processor
        .process_file_buffered(&id, &input, TextBufferOptions::new(1, directory.path()))
        .unwrap();
    let result = processor.process_single_buffered("rpt 2;", result).unwrap();
    assert_eq!(result.into_string().unwrap(), "<👩🏽‍💻\r\né\r\n><👩🏽‍💻\r\né\r\n>");
}

#[test]
fn errors_remain_errors_instead_of_partial_success() {
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    for source in ["atb \"{{missing}}\";", "cblk missing;", "ins 999 x;"] {
        let id = pipeline(&mut processor, source);
        let expected = processor.process_all(&id, "abc").unwrap_err();
        let input =
            TextBuffer::from_text("abc", TextBufferOptions::new(0, directory.path())).unwrap();
        let actual = processor.process_all_buffered(&id, input).unwrap_err();
        assert_eq!(actual.error_code, expected.error_code);
    }
    let input = TextBuffer::from_text("x", TextBufferOptions::new(1, directory.path())).unwrap();
    assert!(processor.process_all_buffered("missing", input).is_err());
    let input = TextBuffer::from_text("x", TextBufferOptions::new(1, directory.path())).unwrap();
    assert!(processor.process_single_buffered(";", input).is_err());
}

#[test]
fn batch_keeps_order_context_isolation_and_errors_and_spills_results() {
    let directory = tempfile::tempdir().unwrap();
    let a = directory.path().join("a.txt");
    let b = directory.path().join("b.txt");
    std::fs::write(&a, "A").unwrap();
    std::fs::write(&b, "B").unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "var name = x;\nate \"{{name}}\";\n");
    let tasks = [
        BufferedTask::new(&id, &a),
        BufferedTask::new(&id, directory.path().join("missing")),
        BufferedTask::new(&id, &b),
    ];
    let results = processor.process_batch_buffered(
        &tasks,
        &TextBufferOptions::new(1024 * 1024, directory.path()),
    );
    assert_eq!(results.len(), 3);
    let mut iter = results.into_iter();
    let first = iter.next().unwrap().unwrap();
    assert!(first.is_spilled());
    assert_eq!(first.into_string().unwrap(), "Ax");
    assert!(iter.next().unwrap().is_err());
    let last = iter.next().unwrap().unwrap();
    assert!(last.is_spilled());
    assert_eq!(last.into_string().unwrap(), "Bx");
}

#[test]
fn dropping_spilled_buffers_cleans_the_storage_directory() {
    let directory = tempfile::tempdir().unwrap();
    {
        let buffer =
            TextBuffer::from_text("text", TextBufferOptions::new(0, directory.path())).unwrap();
        assert!(buffer.is_spilled());
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
}

#[cfg(feature = "bytecode")]
#[test]
fn bytecode_loaded_pipeline_uses_the_same_buffered_executor() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pipeline.textforgebc");
    std::fs::write(&path, []).unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb \"👩🏽‍💻 \";\nrpt 3;\n");
    processor.write_to_bytecode_file(&id, &path).unwrap();
    let loaded = processor.read_from_bytecode_file(&path).unwrap();
    let buffer = TextBuffer::from_text("x", TextBufferOptions::new(1, directory.path())).unwrap();
    assert_eq!(
        processor
            .process_all_buffered(&loaded, buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        processor.process_all(&id, "x").unwrap()
    );
}

#[cfg(feature = "test_access")]
#[test]
fn safe_all_tokens_pipeline_matches_the_reference_executor() {
    let mut processor = TextForgeProcessor::new();
    let id =
        textforge::utils::test_helpers::build_all_tokens_pipeline_safe(&mut processor).unwrap();
    let input = "Banana Laranja cheia de canja";
    let expected = processor.process_all(&id, input).unwrap();
    let buffer =
        TextBuffer::from_text(input, TextBufferOptions::new(1, std::env::temp_dir())).unwrap();
    assert_eq!(
        processor
            .process_all_buffered(&id, buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        expected
    );
}

#[test]
fn generated_mixed_pipelines_match_reference_results() {
    let instructions = [
        "atb Q;", "ate Z;", "rpt 2;", "null;", "raw a z;", "dlf;", "dll;", "rev;", "tbs;", "tua;",
        "tla;",
    ];
    let mut seed = 42u64;
    for _ in 0..100 {
        let mut source = String::new();
        for _ in 0..8 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            source.push_str(instructions[(seed >> 32) as usize % instructions.len()]);
            source.push('\n');
        }
        let mut processor = TextForgeProcessor::new();
        let id = pipeline(&mut processor, &source);
        let input = " a é 👩🏽‍💻 é\r\n";
        let expected = processor.process_all(&id, input).unwrap();
        let buffer = TextBuffer::from_reader(
            input.as_bytes(),
            TextBufferOptions::new(2, std::env::temp_dir()),
        )
        .unwrap();
        assert_eq!(
            processor
                .process_all_buffered(&id, buffer)
                .unwrap()
                .into_string()
                .unwrap(),
            expected,
            "{source}"
        );
    }
}

#[test]
fn byte_range_export_can_reconstruct_utf8_without_a_string_conversion() {
    let text = "👨‍👩‍👧‍👦 é 🇧🇷\0";
    let mut buffer =
        TextBuffer::from_text(text, TextBufferOptions::new(1, std::env::temp_dir())).unwrap();
    let mut exported = Vec::new();
    let mut offset = 0;
    loop {
        let mut chunk = [0; 7];
        let count = buffer.read_at(offset, &mut chunk).unwrap();
        if count == 0 {
            break;
        }
        exported.extend_from_slice(&chunk[..count]);
        offset += count as u64;
    }
    assert_eq!(offset, buffer.len_bytes());
    assert_eq!(exported, text.as_bytes());
}
