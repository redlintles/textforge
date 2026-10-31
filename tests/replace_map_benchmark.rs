//! Informational benchmark for repeated replacement-map workloads.
use std::{hint::black_box, time::Instant};
use textforge::api::{
    TextForgeBuilderMethods,
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};
fn register(processor: &mut TextForgeProcessor) -> String {
    let mut builder = processor.create_pipeline();
    for i in 0..16 {
        builder
            .define_replace_map("m", format!(r"\bword{i}\b"), format!("value{i}"))
            .unwrap();
    }
    builder.execute_replace_map("m").unwrap().build().unwrap()
}
#[test]
#[ignore = "informational release benchmark"]
fn registered_cache_reuse() {
    let input = "word0 word1 word2 word3 word4 word5 word6 word7 ".repeat(32);
    let mut processor = TextForgeProcessor::new();
    let id = register(&mut processor);
    let expected = processor.process_all(&id, &input).unwrap();
    let started = Instant::now();
    for _ in 0..100 {
        assert_eq!(
            black_box(processor.process_all(&id, black_box(&input)).unwrap()),
            expected
        );
    }
    let warm = started.elapsed();
    let started = Instant::now();
    for _ in 0..100 {
        let mut processor = TextForgeProcessor::new();
        let id = register(&mut processor);
        assert_eq!(
            black_box(processor.process_all(&id, black_box(&input)).unwrap()),
            expected
        );
    }
    eprintln!(
        "100 documents, 16 rules: warmed={warm:?}, rebuilt={:?}",
        started.elapsed()
    );
}
