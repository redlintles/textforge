//! Execution profiles change resources, never results (`docs/execution-profiles.md`):
//! every preset, and the smallest accepted spool threshold, give byte-identical
//! output and errors in every processing method.
#![cfg(feature = "test_access")]

use textforge::{
    api::{
        ExecutionProfile, TextForgeBuilderMethods,
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    },
    text::reader::read_from_text,
    utils::test_helpers::build_all_tokens_pipeline_safe,
};

fn profiles() -> Vec<ExecutionProfile> {
    vec![
        ExecutionProfile::balanced(),
        ExecutionProfile::low_memory(),
        ExecutionProfile::fast(),
        ExecutionProfile {
            spool_threshold: ExecutionProfile::MIN_SPOOL_THRESHOLD as usize,
        },
    ]
}

fn pipelines(processor: &mut TextForgeProcessor) -> Vec<String> {
    let text = [
        &[
            "tbs;",
            "onlni 2 '[[__LINES__ - 1]]' apply uniqln;",
            "ate ' {{__LINES__}}/{{__LEN_CHARS__}}';",
        ][..],
        &[
            "rmapd m add 'é' 'e';",
            "rmape m;",
            "blk b assoc iter 3 times onln '[[__COUNTER__ + 1]]' apply tua;",
            "cblk b;",
        ],
        &["dlc 999999 999999;"],
        &["revln;", "joinln ' | ';", "rpt 2;"],
    ];
    let mut ids: Vec<String> = text
        .iter()
        .map(|lines| {
            processor.add_transform(lines.iter().map(|l| read_from_text(l).unwrap()).collect())
        })
        .collect();
    ids.push(build_all_tokens_pipeline_safe(processor).unwrap());
    ids.push(
        processor
            .create_pipeline()
            .trim_both_sides()
            .unwrap()
            .build()
            .unwrap(),
    );
    ids
}

fn inputs() -> Vec<String> {
    let line = "  linha é 👩🏽‍💻 com acento\r\n";
    vec![
        String::new(),
        "a".into(),
        "b\na\nb\na\nc".into(),
        line.repeat(5_000), // ~170 KB: above every small threshold
    ]
}

/// Output (or error code) of every method that has a String result.
fn outcomes(profile: ExecutionProfile) -> Vec<String> {
    let mut processor = TextForgeProcessor::new().with_profile(profile);
    let ids = pipelines(&mut processor);
    let directory = tempfile::tempdir().unwrap();
    let mut results = Vec::new();
    let show = |result: Result<String, textforge::utils::errors::TextForgeError>| match result {
        Ok(text) => text,
        Err(error) => format!("error {:?}", error.error_code),
    };
    for (index, id) in ids.iter().enumerate() {
        for (number, input) in inputs().iter().enumerate() {
            results.push(show(processor.process_all(id, input)));
            results.push(show(processor.process_all_with_debug(id, input)));
            let source = directory.path().join(format!("in-{index}-{number}.txt"));
            let target = directory.path().join(format!("out-{index}-{number}.txt"));
            std::fs::write(&source, input).unwrap();
            results.push(show(
                processor
                    .process_file(id, &source, &target)
                    .map(|()| std::fs::read_to_string(&target).unwrap()),
            ));
        }
    }
    // Batches, in parallel.
    let tasks: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(index, id)| {
            let source = directory.path().join(format!("in-{index}-3.txt"));
            let target = directory.path().join(format!("batch-{index}.txt"));
            (source, id.clone(), target)
        })
        .collect();
    let borrowed = tasks
        .iter()
        .map(|(source, id, target)| (source.as_path(), id.as_str(), target.as_path()))
        .collect();
    for (result, (_, _, target)) in processor.process_batch(borrowed).into_iter().zip(&tasks) {
        results.push(show(
            result.map(|()| std::fs::read_to_string(target).unwrap()),
        ));
    }
    results
}

#[test]
fn every_profile_gives_identical_results() {
    let reference = outcomes(ExecutionProfile::balanced());
    assert!(reference.iter().any(|result| result.starts_with("error")));
    for profile in profiles() {
        let results = outcomes(profile);
        assert_eq!(results.len(), reference.len());
        for (index, (actual, expected)) in results.iter().zip(&reference).enumerate() {
            assert!(
                actual == expected,
                "profile {profile:?}, result {index} differs"
            );
        }
    }
}

#[test]
fn profiles_can_be_changed_and_read_back() {
    let mut processor = TextForgeProcessor::new();
    assert_eq!(processor.profile(), &ExecutionProfile::balanced());
    processor.set_profile(ExecutionProfile::fast());
    assert_eq!(processor.profile(), &ExecutionProfile::fast());

    let mut portable = textforge::api::portable::PortableProcessor::new();
    portable.set_profile(ExecutionProfile::low_memory());
    assert_eq!(portable.profile(), ExecutionProfile::low_memory());
}
