//! Identical batch worker for the mmap versus safe-read experiment.
use std::{error::Error, path::PathBuf};
use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err("usage: batch_io_probe PIPELINE INPUT_DIR OUTPUT_DIR ITERATIONS".into());
    }
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&PathBuf::from(&args[1]))?;
    let inputs: Vec<_> = (0..4)
        .map(|i| PathBuf::from(&args[2]).join(format!("input-{i}.txt")))
        .collect();
    let outputs: Vec<_> = (0..4)
        .map(|i| PathBuf::from(&args[3]).join(format!("output-{i}.txt")))
        .collect();
    for _ in 0..args[4].parse::<usize>()? {
        let tasks = inputs
            .iter()
            .zip(&outputs)
            .map(|(input, output)| (input.as_path(), id.as_str(), output.as_path()))
            .collect();
        for result in processor.process_batch(tasks) {
            result?;
        }
    }
    Ok(())
}
