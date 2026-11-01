//! Identical-source baseline/candidate benchmark; see docs/allocation-optimization.md.
#[path = "support/allocation_workload.rs"]
mod workload;
use std::{hint::black_box, time::Instant};
use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map_or("coverage", String::as_str);
    let cases = workload::cases()?;
    if mode == "fixtures" {
        let inputs = [
            "",
            "x",
            "é",
            "🍌",
            "e\u{301}",
            "ß ΟΣ İ banana\r\n\r\nbanana tail\u{2028}end",
            "  banana  pear\tapple\r\n\r\nbanana\r\ntail",
            "already_normal",
            "\"plain\"",
            "banana banana banana\nsecond\nthird\n",
            "\u{2003} \t",
            "\r\n\r\n",
            "a\u{b}b\u{c}c\u{85}d\u{2029}e",
        ];
        let mut fixtures = Vec::new();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        for case in &cases {
            for input in inputs {
                for owned in [false, true] {
                    let result = std::panic::catch_unwind(|| workload::execute(case, input, owned));
                    let outcome = match result {
                        Ok(Ok(text)) => serde_json::json!({"ok":text}),
                        Ok(Err(error)) => serde_json::json!({"error":format!("{:?}",error)}),
                        Err(_) => serde_json::json!({"panic":true}),
                    };
                    fixtures.push(serde_json::json!({"case":case,"input":input,"owned":owned,"outcome":outcome}));
                }
            }
        }
        std::panic::set_hook(previous);
        println!("{}", serde_json::to_string_pretty(&fixtures)?);
        return Ok(());
    }
    let path = args
        .get(2)
        .ok_or("usage: allocation_benchmark coverage|large INPUT [REPEATS]")?;
    let input = std::fs::read_to_string(path)?;
    let repeats: usize = args.get(3).map_or(Ok(3), |s| s.parse())?;
    let mut processor = TextForgeProcessor::new();
    let mut ids = Vec::new();
    if mode == "coverage" {
        for case in &cases {
            // Guards keep index-based instructions valid on every document.
            let mut guarded = case.clone();
            let mut lines = vec![serde_json::json!(
                "atb 'banana banana banana banana banana banana banana banana\nsecond\nthird\n';"
            )];
            lines.extend(
                case["lines"]
                    .as_array()
                    .ok_or("missing lines")?
                    .iter()
                    .cloned(),
            );
            guarded["lines"] = lines.into();
            ids.push(workload::register(&mut processor, &guarded)?);
        }
    } else if mode == "large" {
        // Preserve document scale between edits; no early selectors or explosive repeats.
        let lines = [
            "tbs;",
            "ate '';",
            "htmle;",
            "htmlu;",
            "rcw NEVER_MATCH x 2;",
            "tlcc 1 8;",
            "tucc 1 8;",
            "tlcs 2;",
            "tucs 2;",
            "ctc 1 8;",
            "clw;",
            "splc;",
            "rmws;",
            "rtl 2;",
            "rtr 2;",
            "jsone;",
            "jsonu;",
            "dlf;",
            "dll;",
            "ins 1 xy;",
            "dlc 1 2;",
        ];
        ids.push(workload::register(
            &mut processor,
            &serde_json::json!({"lines":lines}),
        )?);
    } else {
        return Err("unknown mode".into());
    }
    let started = Instant::now();
    let mut digest = 0xcbf29ce484222325_u64;
    let mut output_bytes = 0_u64;
    for _ in 0..repeats {
        for id in &ids {
            let output =
                processor.run_transform(id, std::borrow::Cow::Borrowed(black_box(&input)))?;
            output_bytes += output.len() as u64;
            for byte in output.bytes() {
                digest = (digest ^ u64::from(byte)).wrapping_mul(0x100000001b3);
            }
            black_box(output);
        }
    }
    println!(
        "{}",
        serde_json::json!({"mode":mode,"input_bytes":input.len(),"pipelines":ids.len(),"repeats":repeats,"elapsed_seconds":started.elapsed().as_secs_f64(),"output_bytes":output_bytes,"digest":format!("{digest:016x}")})
    );
    Ok(())
}
