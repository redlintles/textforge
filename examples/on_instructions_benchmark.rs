//! One benchmark sample per process for `on*` vs whole-document instructions.
//!
//! Usage: on_instructions_benchmark <case> <input-file> <pipeline-dir>
//! Prints one JSON object: case, bytes, lines, seconds, peak RSS (VmHWM, KiB).
//! Driven by `scripts/benchmark_on_instructions.py`; see
//! `docs/benchmarks/on-instructions/README.md`.
use std::{hint::black_box, path::Path, time::Instant};

use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

fn pipeline(case: &str, lines: usize) -> Option<String> {
    let middle = lines / 2;
    let tenth = lines / 10;
    Some(match case {
        "raw_whole" => "raw a A;\n".into(),
        "onlni_whole" => format!("onlni 1 {lines} apply raw a A;\n"),
        "onlni_tenth" => format!("onlni {} {} apply raw a A;\n", middle, middle + tenth - 1),
        "onln_middle" => format!("onln {middle} apply raw a A;\n"),
        "onln_ten" => (1..=10)
            .map(|i| format!("onln {} apply raw a A;\n", i * lines / 11))
            .collect(),
        "onck_middle" => "onck 50000000 50000999 apply raw a A;\n".into(),
        "identity" => "null;\n".into(),
        _ => return None,
    })
}

fn peak_rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmHWM:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|value| value.parse().ok())
        })
        .unwrap_or(0)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, case, input_path, pipeline_dir] = arguments.as_slice() else {
        return Err("usage: on_instructions_benchmark <case> <input> <pipeline-dir>".into());
    };
    let input = std::fs::read_to_string(input_path)?;
    let lines = input.lines().count();
    let source = pipeline(case, lines).ok_or("unknown case")?;
    let pipeline_path = Path::new(pipeline_dir).join(format!("{case}.textforge"));
    std::fs::write(&pipeline_path, source)?;

    let mut processor = TextForgeProcessor::new();
    let id = processor
        .get_pipeline_from_file(&pipeline_path)
        .map_err(|error| format!("{error:?}"))?;
    let started = Instant::now();
    let output = processor
        .process_all(&id, black_box(&input))
        .map_err(|error| format!("{error:?}"))?;
    let seconds = started.elapsed().as_secs_f64();
    black_box(&output);
    println!(
        "{{\"case\":\"{case}\",\"bytes\":{},\"lines\":{lines},\"output_bytes\":{},\"seconds\":{seconds},\"peak_rss_kib\":{}}}",
        input.len(),
        output.len(),
        peak_rss_kib()
    );
    Ok(())
}
