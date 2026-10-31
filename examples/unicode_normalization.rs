//! Run with `cargo run --example unicode_normalization`.
use textforge::api::{
    TextForgeBuilderMethods,
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};
use textforge::utils::errors::TextForgeError;

fn main() -> Result<(), TextForgeError> {
    let mut processor = TextForgeProcessor::new();
    let pipeline = processor
        .create_pipeline()
        .normalize_unicode("nfc")?
        .build()?;
    let output = processor.process_all(&pipeline, "Cafe\u{301}")?;
    assert_eq!(output, "Café");
    println!("{output}");
    Ok(())
}
