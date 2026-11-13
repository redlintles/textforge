//! Cost of the static context and built-in variables (`__LINES__`, …).
//!
//! Usage: builtin_variables_benchmark <case> <samples>
//! Prints one JSON object per case: median seconds over `samples` runs.
//! Driven by `scripts/benchmark_builtin_variables.py`; see
//! `docs/benchmarks/builtin-variables/README.md`. Cases that only use the
//! 0.2.0 DSL also run on an older checkout, for comparison.
use std::{hint::black_box, path::Path, time::Instant};

use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

/// `(pipeline, input kind, process_all calls per sample)`.
fn case(name: &str) -> Option<(&'static str, &'static str, usize)> {
    Some(match name {
        // Per-run overhead: many runs of a short pipeline on a short text.
        "small_runs" => ("tbs;\natb '>';\nate '<';\n", "small", 200_000),
        // Parameter binding with templates, 2000 bindings per run.
        "expression_iter" => ("iter 2000 times ate '[[__COUNTER__ % 10]]';\n", "small", 50),
        "variable_iter" => ("var x = 'a';\niter 2000 times ate '{{x}}';\n", "small", 50),
        // Whole-document work, 100 MB.
        "null_100mb" => ("null;\n", "large", 1),
        "raw_100mb" => ("raw a A;\n", "large", 1),
        "lines_100mb" => ("ate '{{__LINES__}}';\n", "large", 1),
        "len_chars_100mb" => ("ate '{{__LEN_CHARS__}}';\n", "large", 1),
        "lines_twice_100mb" => ("ate '{{__LINES__}} [[__LINES__ + 1]]';\n", "large", 1),
        _ => return None,
    })
}

fn large_input() -> Result<String, std::io::Error> {
    let mut book = String::new();
    for name in ["book1.txt", "book2.txt", "book3.txt"] {
        book.push_str(&std::fs::read_to_string(Path::new("data").join(name))?);
    }
    let mut text = String::with_capacity(105_000_000);
    while text.len() < 100_000_000 {
        text.push_str(&book);
    }
    Ok(text)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = std::env::args().collect();
    let [_, name, samples] = arguments.as_slice() else {
        return Err("usage: builtin_variables_benchmark <case> <samples>".into());
    };
    let (source, kind, calls) = case(name).ok_or("unknown case")?;
    let samples: usize = samples.parse()?;
    let input = match kind {
        "large" => large_input()?,
        _ => "  abc  ".to_owned(),
    };

    let directory = std::env::temp_dir().join(format!("tf-bench-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{name}.textforge"));
    std::fs::write(&path, source)?;
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&path)?;
    std::fs::remove_dir_all(&directory)?;

    let mut times = Vec::with_capacity(samples);
    for _ in 0..samples {
        let start = Instant::now();
        for _ in 0..calls {
            black_box(processor.process_all(&id, black_box(&input))?);
        }
        times.push(start.elapsed().as_secs_f64());
    }
    times.sort_by(f64::total_cmp);
    println!(
        "{{\"case\": \"{name}\", \"calls\": {calls}, \"bytes\": {}, \"median_s\": {:.6}, \"min_s\": {:.6}, \"max_s\": {:.6}}}",
        input.len(),
        times[samples / 2],
        times[0],
        times[samples - 1]
    );
    Ok(())
}
