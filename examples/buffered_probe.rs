//! Controlled experiment runner. Build with --features buffered --example buffered_probe.
#[cfg(not(feature = "buffered"))]
fn main() {
    eprintln!("Build this experiment with --features buffered");
    std::process::exit(2);
}

#[cfg(feature = "buffered")]
fn main() -> Result<(), textforge::utils::errors::TextForgeError> {
    use textforge::utils::errors::{TextForgeError, TextForgeErrorCode};
    fn probe_error(error: impl std::fmt::Display) -> TextForgeError {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError(error.to_string().into()),
            "buffered_probe",
            "experiment runner",
        )
    }
    use std::fs::File;
    use std::path::Path;
    use textforge::api::TextBufferOptions;
    use textforge::api::processor::{BufferedTask, TextForgeProcessor, TextForgeProcessorMethods};
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 7 {
        return Err(probe_error(
            "usage: buffered_probe MODE PIPELINE INPUT OUTPUT SPILL_DIR THRESHOLD [BATCH_COUNT]",
        ));
    }
    let mode = &args[1];
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(Path::new(&args[2]))?;
    let options = TextBufferOptions::new(args[6].parse().map_err(probe_error)?, &args[5]);
    let count: usize = args
        .get(7)
        .map(|x| x.parse())
        .transpose()
        .map_err(probe_error)?
        .unwrap_or(4);
    match mode.as_str() {
        "legacy" => {
            let input = std::fs::read_to_string(&args[3]).map_err(probe_error)?;
            let result = processor.process_all(&id, &input)?;
            std::fs::write(&args[4], result).map_err(probe_error)?;
        }
        "buffered" | "buffered-string" => {
            let mut result = processor.process_file_buffered(&id, &args[3], options)?;
            eprintln!(
                "bytes={} spilled={}",
                result.len_bytes(),
                result.is_spilled()
            );
            if mode == "buffered-string" {
                std::fs::write(&args[4], result.into_string()?).map_err(probe_error)?;
            } else {
                result.copy_to(File::create(&args[4]).map_err(probe_error)?)?;
            }
        }
        "legacy-batch" | "buffered-batch" => {
            let paths: Vec<_> = (0..count)
                .map(|i| Path::new(&args[4]).join(format!("result-{i}.txt")))
                .collect();
            if mode == "legacy-batch" {
                let tasks = paths
                    .iter()
                    .map(|out| (Path::new(&args[3]), id.as_str(), out.as_path()))
                    .collect();
                for result in processor.process_batch(tasks) {
                    result?;
                }
            } else {
                let tasks: Vec<_> = (0..count)
                    .map(|_| BufferedTask::new(&id, &args[3]))
                    .collect();
                let results = processor.process_batch_buffered(&tasks, &options);
                for (result, path) in results.into_iter().zip(paths) {
                    result?.copy_to(File::create(path).map_err(probe_error)?)?;
                }
            }
        }
        _ => return Err(probe_error("unknown mode")),
    }
    Ok(())
}
