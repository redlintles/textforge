//! Release-mode worker for scripts/benchmark_comparison.py.
use std::{fs, path::Path};
use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    utils::errors::{TextForgeError, TextForgeErrorCode},
};

fn failure(e: impl std::fmt::Display) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::ValidationError(e.to_string().into()),
        "comparison_probe",
        "benchmark",
    )
}

fn main() -> Result<(), TextForgeError> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 7 {
        return Err(failure(
            "usage: comparison_probe METHOD PIPELINE INPUT OUTPUT ITERATIONS BATCH_SIZE",
        ));
    }
    let iterations: usize = args[5].parse().map_err(failure)?;
    let count: usize = args[6].parse().map_err(failure)?;
    #[cfg(feature = "watchers")]
    if args[1] == "collect" {
        return run_collection(&args[3], &args[4], iterations, count);
    }
    #[cfg(feature = "watchers")]
    if args[1] == "collect-full" {
        return run_full_statistics(&args[3], &args[4], iterations, count);
    }
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(Path::new(&args[2]))?;
    let paths: Vec<_> = (0..count)
        .map(|i| Path::new(&args[4]).join(format!("result-{i}.txt")))
        .collect();
    #[cfg(feature = "watchers")]
    let mut watchers = benchmark_watchers()?;
    for _ in 0..iterations {
        match args[1].as_str() {
            #[cfg(feature = "watchers")]
            "process_all_with_watchers" => {
                let input = fs::read_to_string(&args[3]).map_err(failure)?;
                let report_path = Path::new(&args[4]).join("watchers.json");
                let output = processor.process_all_with_watchers(
                    &id,
                    &input,
                    &mut watchers,
                    &report_path,
                )?;
                std::hint::black_box(&output);
                fs::write(&paths[0], output).map_err(failure)?;
            }
            "process_batch" => {
                let tasks = paths
                    .iter()
                    .map(|out| (Path::new(&args[3]), id.as_str(), out.as_path()))
                    .collect();
                for result in processor.process_batch(tasks) {
                    result?;
                }
            }
            "process_all" | "process_all_with_debug" => {
                let input = fs::read_to_string(&args[3]).map_err(failure)?;
                let output = if args[1] == "process_all" {
                    processor.process_all(&id, &input)?
                } else {
                    processor.process_all_with_debug(&id, &input)?
                };
                std::hint::black_box(&output);
                fs::write(&paths[0], output).map_err(failure)?;
            }
            _ => return Err(failure("unknown method")),
        }
    }
    Ok(())
}

/// Representative diagnostics covering every WatcherContext field.
#[cfg(feature = "watchers")]
fn benchmark_watchers() -> Result<textforge::watchers::WatcherList, TextForgeError> {
    use textforge::watchers::{ExecutionWindows, WatcherContext, WatcherList, default_watchers};
    let mut list = WatcherList::new();
    list.set_watcher("bytes", default_watchers::current_len_bytes);
    list.set_watcher("characters", default_watchers::current_len_chars);
    list.set_watcher("words", default_watchers::word_count);
    list.set_watcher("lines", default_watchers::line_count);
    list.set_watcher("byte_delta", default_watchers::byte_delta);
    list.set_watcher("unchanged", default_watchers::is_unchanged);
    list.set_watcher("next_bytes", |ctx: WatcherContext| {
        ctx.after
            .as_ref()
            .map_or_else(|| "none".to_string(), |text| text.len().to_string())
    });
    list.set_watcher("instruction", |ctx: WatcherContext| {
        ctx.instruction.to_string()
    });
    for name in [
        "bytes",
        "characters",
        "words",
        "lines",
        "byte_delta",
        "unchanged",
        "next_bytes",
        "instruction",
    ] {
        list.schedule_watcher(name, ExecutionWindows::All)?;
    }
    Ok(list)
}

/// Collection-only control: fixed shared snapshots and cheap metadata watchers.
#[cfg(feature = "watchers")]
fn run_collection(
    input_path: &str,
    output: &str,
    repetitions: usize,
    steps: usize,
) -> Result<(), TextForgeError> {
    use std::sync::Arc;
    use textforge::watchers::{ExecutionWindows, WatcherContext, WatcherList};
    let input: Arc<str> = Arc::from(fs::read_to_string(input_path).map_err(failure)?);
    let context = WatcherContext::new(
        Arc::clone(&input),
        Arc::clone(&input),
        Some(Arc::clone(&input)),
        "null",
    );
    let mut list = WatcherList::new();
    list.set_watcher("bytes", |ctx| ctx.current.len().to_string());
    list.set_watcher("before_bytes", |ctx| ctx.before.len().to_string());
    list.set_watcher("next_bytes", |ctx| {
        ctx.after
            .as_ref()
            .map_or_else(|| "none".into(), |text| text.len().to_string())
    });
    list.set_watcher("byte_delta", |ctx| {
        (ctx.current.len() as i64 - ctx.before.len() as i64).to_string()
    });
    list.set_watcher("empty", |ctx| ctx.current.is_empty().to_string());
    list.set_watcher("instruction", |ctx| ctx.instruction.to_string());
    list.set_watcher("prefix", |ctx| ctx.current.starts_with("2026").to_string());
    list.set_watcher("suffix", |ctx| ctx.current.ends_with('\n').to_string());
    for name in [
        "bytes",
        "before_bytes",
        "next_bytes",
        "byte_delta",
        "empty",
        "instruction",
        "prefix",
        "suffix",
    ] {
        list.schedule_watcher(name, ExecutionWindows::All)?;
    }
    for _ in 0..repetitions {
        for _ in 0..steps {
            list.run_watchers(context.clone())?;
        }
        list.to_json(&Path::new(output).join("watchers.json"))?;
        list.reset();
    }
    fs::write(Path::new(output).join("result-0.txt"), input.as_bytes()).map_err(failure)?;
    Ok(())
}

/// Scan-only control using the same eight watchers and immutable shared snapshots.
#[cfg(feature = "watchers")]
fn run_full_statistics(
    input_path: &str,
    output: &str,
    repetitions: usize,
    steps: usize,
) -> Result<(), TextForgeError> {
    use std::sync::Arc;
    use textforge::watchers::WatcherContext;
    let input: Arc<str> = Arc::from(fs::read_to_string(input_path).map_err(failure)?);
    let context = WatcherContext::new(
        Arc::clone(&input),
        Arc::clone(&input),
        Some(Arc::clone(&input)),
        "null",
    );
    let mut list = benchmark_watchers()?;
    for _ in 0..repetitions {
        for _ in 0..steps {
            list.run_watchers(context.clone())?;
        }
        list.to_json(&Path::new(output).join("watchers.json"))?;
        list.reset();
    }
    fs::write(Path::new(output).join("result-0.txt"), input.as_bytes()).map_err(failure)?;
    Ok(())
}
