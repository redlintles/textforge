//! Engine benchmark: parsing, loading, per-step execution overhead, parameter
//! and variable resolution, expressions, blocks, sections, whole documents and
//! batches. Only the public API, so it also builds on older checkouts.
//!
//! Usage: engine_benchmark <case> <samples>   (prints one JSON line)
//! Driven by `scripts/benchmark_engine.py`; see `docs/benchmarks/engine/README.md`.
use std::{hint::black_box, path::Path, time::Instant};

use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

/// Cheap instructions with literal parameters, cycled to build long pipelines.
const LITERAL: &[&str] = &[
    "ate '!';", "atb '>';", "tua;", "tla;", "rev;", "raw a b;", "dlf;", "dll;", "tbs;", "rfw b c;",
    "ins 1 x;", "tucs 0;",
];

fn lines(pattern: &[&str], count: usize) -> String {
    (0..count)
        .map(|i| pattern[i % pattern.len()])
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

/// `(pipeline source, input, process_all calls per sample, load_only)`.
fn case(name: &str) -> Option<(String, String, usize, bool)> {
    let small = "o rato roeu a roupa do rei de roma".to_owned();
    Some(match name {
        // Loading
        "parse_text_5k" => (lines(LITERAL, 5_000), String::new(), 1, true),
        "load_bytecode_5k" => (lines(LITERAL, 5_000), String::new(), 1, true),
        // Execution overhead per run and per step
        "small_runs" => (
            lines(&["tbs;", "atb '>';", "ate '<';"], 3),
            small,
            200_000,
            false,
        ),
        "long_literal_pipeline" => (lines(LITERAL, 300), small, 2_000, false),
        // Parameter resolution
        "variable_params" => (
            format!(
                "var x = 'a';\nvar n = 1;\n{}",
                lines(&["ate {{x}};", "dls {{n}};"], 300)
            ),
            small,
            2_000,
            false,
        ),
        "template_params" => (
            format!(
                "var x = 'a';\n{}",
                lines(&["ate 'v={{x}};';", "dls '{{x}}{{x}}';"], 100)
                    .replace("dls '{{x}}{{x}}';", "rfw '{{x}}' '{{x}}{{x}}';")
            ),
            small,
            2_000,
            false,
        ),
        "expression_params" => (
            lines(&["ate '[[1 + 2 * 3]]';", "dls '[[2 - 1]]';"], 300),
            small,
            2_000,
            false,
        ),
        "iter_expression" => (
            "blk b assoc iter 20000 times onln '[[__COUNTER__ % 3 + 1]]' apply tua;\ncblk b;\n"
                .into(),
            "a\nb\nc".into(),
            20,
            false,
        ),
        "iter_literal" => ("iter 20000 times tua;\n".into(), small, 20, false),
        // Control flow
        "blocks" => (
            format!(
                "{}cblk b;\ncblk b;\n",
                lines(&["blk b assoc ate '!';", "blk b assoc dlf;"], 60)
            ),
            small,
            5_000,
            false,
        ),
        "variables_churn" => (
            lines(
                &[
                    "var a = 1;",
                    "mutv a = 2;",
                    "val c = 3;",
                    "eval 'a + c' in d;",
                ],
                200,
            ),
            small,
            2_000,
            false,
        ),
        // Whole documents and batches
        "raw_100mb" => ("raw a A;\n".into(), String::new(), 1, false),
        "batch_64x1mb" => ("tua;\nraw A b;\n".into(), String::new(), 1, false),
        _ => return None,
    })
}

fn big() -> Result<String, std::io::Error> {
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
        return Err("usage: engine_benchmark <case> <samples>".into());
    };
    let (source, mut input, calls, load_only) = case(name).ok_or("unknown case")?;
    let samples: usize = samples.parse()?;
    if name == "raw_100mb" {
        input = big()?;
    }
    let directory = std::env::temp_dir().join(format!("tf-engine-{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let path = directory.join("pipeline.textforge");
    std::fs::write(&path, &source)?;
    let mut processor = TextForgeProcessor::new();
    let id = processor.get_pipeline_from_file(&path)?;

    let mut times = Vec::with_capacity(samples);
    if name == "batch_64x1mb" {
        let chunk = big()?[..1_000_000].to_owned();
        let inputs: Vec<_> = (0..64)
            .map(|i| directory.join(format!("in{i}.txt")))
            .collect();
        let outputs: Vec<_> = (0..64)
            .map(|i| directory.join(format!("out{i}.txt")))
            .collect();
        for path in &inputs {
            std::fs::write(path, &chunk)?;
        }
        for _ in 0..samples {
            let tasks = inputs
                .iter()
                .zip(&outputs)
                .map(|(i, o)| (i.as_path(), id.as_str(), o.as_path()))
                .collect();
            let start = Instant::now();
            for result in processor.process_batch(tasks) {
                result?;
            }
            times.push(start.elapsed().as_secs_f64());
        }
    } else if load_only {
        #[cfg(feature = "bytecode")]
        let bytecode = directory.join("pipeline.textforgebc");
        #[cfg(feature = "bytecode")]
        {
            std::fs::write(&bytecode, "")?;
            processor.write_to_bytecode_file(&id, &bytecode)?;
        }
        for _ in 0..samples {
            let start = Instant::now();
            for _ in 0..20 {
                let loaded = if name == "load_bytecode_5k" {
                    #[cfg(feature = "bytecode")]
                    {
                        processor.read_from_bytecode_file(&bytecode)?
                    }
                    #[cfg(not(feature = "bytecode"))]
                    {
                        return Err("needs the bytecode feature".into());
                    }
                } else {
                    processor.get_pipeline_from_file(&path)?
                };
                processor.remove_transform(&loaded)?;
            }
            times.push(start.elapsed().as_secs_f64());
        }
    } else {
        for _ in 0..samples {
            let start = Instant::now();
            for _ in 0..calls {
                black_box(processor.process_all(&id, black_box(&input))?);
            }
            times.push(start.elapsed().as_secs_f64());
        }
    }
    std::fs::remove_dir_all(&directory)?;
    times.sort_by(f64::total_cmp);
    println!(
        "{{\"case\": \"{name}\", \"median_s\": {:.6}, \"min_s\": {:.6}}}",
        times[samples / 2],
        times[0]
    );
    Ok(())
}
