//! Release worker for the safe whole-document I/O comparison.
use std::{error::Error, path::PathBuf};
use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        return Err(
            "usage: safe_file_io_probe METHOD PIPELINE INPUT_DIR OUTPUT_DIR ITERATIONS THREADS"
                .into(),
        );
    }
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&PathBuf::from(&args[2]))?;
    let inputs: Vec<_> = (0..4)
        .map(|i| PathBuf::from(&args[3]).join(format!("input-{i}.txt")))
        .collect();
    let outputs: Vec<_> = (0..4)
        .map(|i| PathBuf::from(&args[4]).join(format!("output-{i}.txt")))
        .collect();
    let iterations = args[5].parse::<usize>()?;
    match args[1].as_str() {
        "file" => {
            for _ in 0..iterations {
                for (input, output) in inputs.iter().zip(&outputs) {
                    processor.process_file(&id, input, output)?;
                }
            }
        }
        "batch" => {
            // Both builds use the same caller-owned pool. The new convenience
            // method delegates to this exact install + process_batch sequence.
            let pool = rayon::ThreadPoolBuilder::new()
                .num_threads(args[6].parse()?)
                .build()?;
            for _ in 0..iterations {
                let tasks = inputs
                    .iter()
                    .zip(&outputs)
                    .map(|(input, output)| (input.as_path(), id.as_str(), output.as_path()))
                    .collect();
                for result in pool.install(|| processor.process_batch(tasks)) {
                    result?;
                }
            }
        }
        _ => return Err("unknown method".into()),
    }
    Ok(())
}
