//! Reproducible audit harness; prototypes here are NOT production implementations.
//! See docs/benchmarks/line-performance/report.md for scope and limitations.
use std::{borrow::Cow, hint::black_box, time::Instant};
use textforge::{
    tokens::{InstructionMethods, transforms::*},
    utils::errors::TextForgeError,
};

/// Reference scanner copied from src/utils/lines.rs at the audited revision.
fn is_break(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}
fn lines(mut text: &str) -> impl Iterator<Item = (&str, &str)> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        if let Some((i, c)) = text.char_indices().find(|(_, c)| is_break(*c)) {
            let size = if c == '\r' && text[i..].starts_with("\r\n") {
                2
            } else {
                c.len_utf8()
            };
            let result = (&text[..i], &text[i..i + size]);
            text = &text[i + size..];
            Some(result)
        } else {
            let result = (text, "");
            text = "";
            Some(result)
        }
    })
}
/// Safe byte-scanner experiment, preserving every recognized Unicode separator.
fn byte_lines(mut text: &str) -> impl Iterator<Item = (&str, &str)> {
    std::iter::from_fn(move || {
        if text.is_empty() {
            return None;
        }
        let bytes = text.as_bytes();
        for (i, byte) in bytes.iter().copied().enumerate() {
            let size = match byte {
                b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
                b'\n' | b'\r' | 0xb | 0xc => 1,
                0xc2 if bytes.get(i + 1) == Some(&0x85) => 2,
                0xe2 if bytes.get(i + 1) == Some(&0x80)
                    && matches!(bytes.get(i + 2), Some(0xa8 | 0xa9)) =>
                {
                    3
                }
                _ => continue,
            };
            let result = (&text[..i], &text[i..i + size]);
            text = &text[i + size..];
            return Some(result);
        }
        let result = (text, "");
        text = "";
        Some(result)
    })
}
fn span(text: &str, start: usize, end: usize) -> Option<(usize, usize)> {
    if start == 0 || start > end {
        return None;
    }
    let mut offset = 0;
    let mut begin = 0;
    for (i, (content, ending)) in lines(text).enumerate() {
        if i + 1 == start {
            begin = offset;
        }
        offset += content.len() + ending.len();
        if i + 1 == end {
            return Some((begin, offset));
        }
    }
    None
}
/// Single bounded search, then one bulk removal. String::drain is safe and stable.
fn delete(input: Cow<'_, str>, line: usize) -> Result<Cow<'_, str>, TextForgeError> {
    let (start, end) = span(&input, line, line).ok_or_else(|| failure("Missing line"))?;
    Ok(match input {
        Cow::Owned(mut text) => {
            text.drain(start..end);
            Cow::Owned(text)
        }
        Cow::Borrowed(text) if start == 0 => Cow::Borrowed(&text[end..]),
        Cow::Borrowed(text) if end == text.len() => Cow::Borrowed(&text[..start]),
        Cow::Borrowed(text) => {
            let mut out = String::with_capacity(text.len() - (end - start));
            out.push_str(&text[..start]);
            out.push_str(&text[end..]);
            out.into()
        }
    })
}
/// Borrow a retained range, or truncate before moving its prefix for owned input.
fn select(input: Cow<'_, str>, start: usize, end: usize) -> Result<Cow<'_, str>, TextForgeError> {
    let (begin, finish) = span(&input, start, end).ok_or_else(|| failure("Missing range"))?;
    Ok(match input {
        Cow::Borrowed(text) => Cow::Borrowed(&text[begin..finish]),
        Cow::Owned(mut text) => {
            text.truncate(finish);
            text.drain(..begin);
            text.into()
        }
    })
}
fn append<'a>(input: Cow<'a, str>, payload: &str) -> Cow<'a, str> {
    let ending = lines(&input)
        .find_map(|(_, e)| (!e.is_empty()).then_some(e))
        .unwrap_or("\n");
    // Only eight recognized separator encodings, at most three bytes long.
    let mut ending_bytes = [0u8; 3];
    ending_bytes[..ending.len()].copy_from_slice(ending.as_bytes());
    let ending_len = ending.len();
    let terminated = input.chars().next_back().is_some_and(is_break);
    let before = !input.is_empty() && !terminated;
    let after = (!input.is_empty() && terminated) || payload.is_empty();
    let extra = payload.len() + usize::from(before) * ending_len + usize::from(after) * ending_len;
    let mut output = match input {
        Cow::Owned(mut text) => {
            text.reserve_exact(extra);
            text
        }
        Cow::Borrowed(text) => {
            let mut out = String::with_capacity(text.len() + extra);
            out.push_str(text);
            out
        }
    };
    // Copying the original valid terminator through this checked conversion avoids
    // borrowing the original String while mutating it. No unsafe byte conversion.
    if let Ok(ending) = std::str::from_utf8(&ending_bytes[..ending_len]) {
        if before {
            output.push_str(ending);
        }
        output.push_str(payload);
        if after {
            output.push_str(ending);
        }
    }
    output.into()
}
fn remove_breaks(input: Cow<'_, str>) -> Cow<'_, str> {
    if !input.chars().any(is_break) {
        return input;
    }
    match input {
        Cow::Owned(mut text) => {
            text.retain(|c| !is_break(c));
            text.into()
        }
        Cow::Borrowed(text) => {
            let mut output = String::with_capacity(text.len());
            for (content, _) in lines(text) {
                output.push_str(content);
            }
            output.into()
        }
    }
}
/// Reverse contents without storing a Vec of line descriptors; keep separators
/// at their original positions, including the production EOF blank-line rule.
fn reverse_contents(text: &str) -> impl Iterator<Item = &str> {
    let mut remaining = if text.is_empty() {
        None
    } else {
        let suffix = text.chars().next_back();
        let trim = if text.ends_with("\r\n") {
            2
        } else {
            suffix.filter(|c| is_break(*c)).map_or(0, char::len_utf8)
        };
        Some(&text[..text.len() - trim])
    };
    std::iter::from_fn(move || {
        let current = remaining.take()?;
        if let Some((i, c)) = current.char_indices().rev().find(|(_, c)| is_break(*c)) {
            let begin = if c == '\n' && i > 0 && current.as_bytes()[i - 1] == b'\r' {
                i - 1
            } else {
                i
            };
            remaining = Some(&current[..begin]);
            Some(&current[i + c.len_utf8()..])
        } else {
            Some(current)
        }
    })
}
fn reverse(input: Cow<'_, str>) -> Cow<'_, str> {
    let mut output = String::with_capacity(input.len());
    for (content, (_, ending)) in reverse_contents(&input).zip(lines(&input)) {
        output.push_str(content);
        output.push_str(ending);
    }
    if input.chars().next().is_some_and(is_break)
        && !input.chars().next_back().is_some_and(is_break)
    {
        output.push_str(
            lines(&input)
                .find_map(|(_, e)| (!e.is_empty()).then_some(e))
                .unwrap_or("\n"),
        );
    }
    output.into()
}
/// Compact index experiment: one usize boundary per line, rather than two &strs.
fn reverse_offsets(input: Cow<'_, str>) -> Cow<'_, str> {
    let mut ends = Vec::new();
    let mut offset = 0;
    for (text, ending) in lines(&input) {
        offset += text.len() + ending.len();
        ends.push(offset);
    }
    let mut output = String::with_capacity(input.len());
    for position in 0..ends.len() {
        let other = ends.len() - 1 - position;
        let begin = if other == 0 { 0 } else { ends[other - 1] };
        let raw = &input[begin..ends[other]];
        output.push_str(&raw[..raw.len() - last_ending(raw)]);
        let begin = if position == 0 { 0 } else { ends[position - 1] };
        let forward = &input[begin..ends[position]];
        output.push_str(&forward[forward.len() - last_ending(forward)..]);
    }
    if input.chars().next().is_some_and(is_break)
        && !input.chars().next_back().is_some_and(is_break)
    {
        output.push_str(
            lines(&input)
                .find_map(|(_, e)| (!e.is_empty()).then_some(e))
                .unwrap_or("\n"),
        );
    }
    output.into()
}
fn last_ending(text: &str) -> usize {
    if text.ends_with("\r\n") {
        2
    } else {
        text.chars()
            .next_back()
            .filter(|c| is_break(*c))
            .map_or(0, char::len_utf8)
    }
}
/// Exact-copy alternative releases an oversized owned source after a small cut.
fn select_exact(
    input: Cow<'_, str>,
    start: usize,
    end: usize,
) -> Result<Cow<'_, str>, TextForgeError> {
    let (begin, finish) = span(&input, start, end).ok_or_else(|| failure("Missing range"))?;
    Ok(Cow::Owned(input[begin..finish].to_owned()))
}
fn chain<'a>(
    variant: &str,
    mut input: Cow<'a, str>,
    steps: usize,
) -> Result<Cow<'a, str>, TextForgeError> {
    let token = dln::Dln::new(2)?;
    for _ in 0..steps {
        input = if variant == "chain_base" {
            token.transform(input, None)?
        } else {
            delete(input, 2)?
        };
    }
    Ok(input)
}
fn failure(message: &str) -> TextForgeError {
    use textforge::utils::errors::TextForgeErrorCode;
    TextForgeError::new(
        TextForgeErrorCode::InvalidParameters(message.to_owned().into()),
        "audit prototype",
        "",
    )
}
fn baseline(case: &str, count: usize) -> Result<Box<dyn InstructionMethods>, TextForgeError> {
    Ok(match case {
        "apl" => Box::new(apl::Apl::new("added")?),
        "dln" => Box::new(dln::Dln::new(2)?),
        "dln_last" => Box::new(dln::Dln::new(count)?),
        "dlna" => Box::new(dlna::Dlna::new(2)?),
        "dlna_last" => Box::new(dlna::Dlna::new(count)?),
        "dlnb" => Box::new(dlnb::Dlnb::new(2)?),
        "dlnb_first" => Box::new(dlnb::Dlnb::new(1)?),
        "slnc" => Box::new(slnc::Slnc::new(2, 3)?),
        "slnc_all" => Box::new(slnc::Slnc::new(1, count)?),
        "dlni" => Box::new(dlni::Dlni::new(2, 3)?),
        "inl" => Box::new(inl::Inl::new(2, "added")?),
        "inl_end" => Box::new(inl::Inl::new(count + 1, "added")?),
        "rln" => Box::new(rln::Rln::new(2, "replaced")?),
        "pln" => Box::new(pln::Pln::new("added")?),
        "nln" => Box::new(nln::Nln::new()?),
        "dle" => Box::new(dle::Dle::new()?),
        "klm" => Box::new(klm::Klm::new("^keep")?),
        "klm_all" => Box::new(klm::Klm::new(".*")?),
        "dlm" => Box::new(dlm::Dlm::new("^keep")?),
        "dlm_none" => Box::new(dlm::Dlm::new("^!never")?),
        "revln" => Box::new(revln::Revln::new()?),
        "uniqln" => Box::new(uniqln::Uniqln::new()?),
        "joinln" => Box::new(joinln::Joinln::new(" ")?),
        value if value.starts_with("batch_") => Box::new(dlni::Dlni::new(
            2,
            value[6..]
                .parse::<usize>()
                .map_err(|_| failure("Invalid batch"))?
                + 1,
        )?),
        _ => return Err(failure("Unknown baseline case")),
    })
}
fn prototype<'a>(
    case: &str,
    input: Cow<'a, str>,
    count: usize,
) -> Result<Cow<'a, str>, TextForgeError> {
    match case {
        "dln" => delete(input, 2),
        "dln_last" => delete(input, count),
        "dlna" => select(input, 1, 2),
        "dlna_last" | "slnc_all" => select(input, 1, count),
        "slnc" => select(input, 2, 3),
        "apl" | "inl_end" => Ok(append(input, "added")),
        "nln" => Ok(remove_breaks(input)),
        "revln" => Ok(reverse(input)),
        "revln_offsets" => Ok(reverse_offsets(input)),
        "select_exact" => select_exact(input, 2, 3),
        _ => Err(failure("Unknown prototype case")),
    }
}
fn fixture(kind: &str, bytes: usize) -> Result<String, TextForgeError> {
    if kind == "single" {
        return Ok("a".repeat(bytes));
    }
    if kind == "blank" {
        return Ok("\n".repeat(bytes));
    }
    let mut text = String::with_capacity(bytes);
    let endings = [
        "\n", "\r\n", "\r", "\u{b}", "\u{c}", "\u{85}", "\u{2028}", "\u{2029}",
    ];
    let mut index = 0;
    loop {
        let ending = if kind == "unicode" {
            endings[index % endings.len()]
        } else {
            "\n"
        };
        let width = if kind == "wide" { 4096 } else { 64 };
        let mut line = if kind == "repeat" {
            "keep repeated ".to_owned()
        } else {
            format!(
                "{} {index:08x} ",
                if index % 2 == 0 { "keep" } else { "drop" }
            )
        };
        if kind == "unicode" {
            line.push_str("é🍌e\u{301} © 👩🏽‍💻 ");
        }
        while line.len() + ending.len() < width {
            line.push('x');
        }
        line.push_str(ending);
        if text.len() + line.len() > bytes {
            break;
        }
        text.push_str(&line);
        index += 1;
    }
    // Remove generator spare capacity before comparing owned buffers fairly.
    Ok(text.into_boxed_str().into_string())
}
fn owned(text: &str, mode: &str) -> String {
    let mut value = String::with_capacity(text.len() + if mode == "spare" { 256 } else { 0 });
    value.push_str(text);
    value
}
fn verify() -> Result<(), Box<dyn std::error::Error>> {
    let atoms = [
        "a",
        "é🍌e\u{301}",
        "© 👩🏽‍💻",
        "\n",
        "\r",
        "\r\n",
        "\u{b}",
        "\u{c}",
        "\u{85}",
        "\u{2028}",
        "\u{2029}",
    ];
    let mut checked = 0;
    for code in 0..=14641 {
        let mut state = code;
        let mut input = String::new();
        for _ in 0..4 {
            input.push_str(atoms[state % atoms.len()]);
            state /= atoms.len();
        }
        if code == 14641 {
            input.clear();
        }
        let reference: Vec<_> = lines(&input).collect();
        let fast: Vec<_> = byte_lines(&input).collect();
        if reference != fast {
            return Err("Byte scanner semantic mismatch".into());
        }
        let count = reference.len();
        for mode in ["borrowed", "owned"] {
            let argument = || {
                if mode == "borrowed" {
                    Cow::Borrowed(input.as_str())
                } else {
                    Cow::Owned(input.clone())
                }
            };
            for line in 1..=count {
                if dln::Dln::new(line)?.transform(argument(), None)? != delete(argument(), line)? {
                    return Err("Delete mismatch".into());
                }
                for end in line..=count {
                    if slnc::Slnc::new(line, end)?.transform(argument(), None)?
                        != select(argument(), line, end)?
                    {
                        return Err("Select mismatch".into());
                    }
                }
            }
            for payload in ["", "added", "👩🏽‍💻e\u{301}"] {
                if apl::Apl::new(payload)?.transform(argument(), None)?
                    != append(argument(), payload)
                {
                    return Err("Append mismatch".into());
                }
            }
            if nln::Nln::new()?.transform(argument(), None)? != remove_breaks(argument()) {
                return Err("Remove breaks mismatch".into());
            }
            if revln::Revln::new()?.transform(argument(), None)? != reverse(argument()) {
                return Err("Reverse mismatch".into());
            }
            if revln::Revln::new()?.transform(argument(), None)? != reverse_offsets(argument()) {
                return Err("Offset reverse mismatch".into());
            }
            if count >= 3
                && slnc::Slnc::new(2, 3)?.transform(argument(), None)?
                    != select_exact(argument(), 2, 3)?
            {
                return Err("Exact selection mismatch".into());
            }
            checked += 1;
        }
    }
    let text = fixture("ascii", 64000)?;
    for steps in [10, 50] {
        for owned in [false, true] {
            let argument = || {
                if owned {
                    Cow::Owned(text.clone())
                } else {
                    Cow::Borrowed(text.as_str())
                }
            };
            let expected = chain("chain_base", argument(), steps)?;
            if chain("chain_proto", argument(), steps)? != expected
                || dlni::Dlni::new(2, steps + 1)?.transform(argument(), None)? != expected
            {
                return Err("Chain mismatch".into());
            }
        }
    }
    println!("{{\"verified_inputs_modes\":{checked},\"atom_combinations\":14642}}");
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("verify") {
        return verify();
    }
    if args.len() != 8 {
        return Err(
            "usage: binary variant case dataset bytes ownership iterations timing|rss".into(),
        );
    }
    let (variant, case, kind, mode) = (&args[1], &args[2], &args[3], &args[5]);
    let bytes: usize = args[4].parse()?;
    let iterations: usize = args[6].parse()?;
    let measurement = &args[7];
    let source = fixture(kind, bytes)?;
    let count = lines(&source).count();
    let length = source.len();
    let token = if variant == "baseline" {
        Some(baseline(case, count)?)
    } else {
        None
    };
    if measurement == "rss" {
        let start = Instant::now();
        let output = if case == "input_only" {
            Cow::Owned(source)
        } else if mode == "borrowed" {
            if let Some(token) = &token {
                token.transform(Cow::Borrowed(&source), None)?
            } else {
                prototype(case, Cow::Borrowed(&source), count)?
            }
        } else {
            let argument = if mode == "spare" {
                let mut source = source;
                source.reserve_exact(256);
                Cow::Owned(source)
            } else {
                Cow::Owned(source)
            };
            if let Some(token) = &token {
                token.transform(argument, None)?
            } else {
                prototype(case, argument, count)?
            }
        };
        let elapsed = start.elapsed().as_nanos();
        print_result(
            variant, case, kind, mode, length, count, iterations, elapsed, &output, false,
        );
        black_box(output);
        return Ok(());
    }
    let mut total = 0;
    let mut last_len = 0;
    let mut capacity = 0;
    let mut borrowed = false;
    let mut reuse = false;
    for _ in 0..iterations {
        let argument = if mode == "borrowed" {
            Cow::Borrowed(source.as_str())
        } else {
            Cow::Owned(owned(&source, mode))
        };
        let pointer = argument.as_ptr();
        let start = Instant::now();
        let output = if variant.starts_with("chain_") {
            chain(variant, black_box(argument), case.parse()?)?
        } else if variant == "scan_chars" {
            Cow::Owned(
                lines(black_box(&source))
                    .map(|(t, e)| t.len() + e.len())
                    .sum::<usize>()
                    .to_string(),
            )
        } else if variant == "scan_bytes" {
            Cow::Owned(
                byte_lines(black_box(&source))
                    .map(|(t, e)| t.len() + e.len())
                    .sum::<usize>()
                    .to_string(),
            )
        } else if let Some(token) = &token {
            token.transform(black_box(argument), None)?
        } else {
            prototype(case, black_box(argument), count)?
        };
        total += start.elapsed().as_nanos();
        last_len = output.len();
        borrowed = matches!(output, Cow::Borrowed(_));
        reuse = output.as_ptr() == pointer;
        capacity = match &output {
            Cow::Owned(value) => value.capacity(),
            Cow::Borrowed(_) => 0,
        };
        black_box(&output);
    }
    println!(
        "{{\"variant\":\"{variant}\",\"case\":\"{case}\",\"dataset\":\"{kind}\",\"ownership\":\"{mode}\",\"bytes\":{length},\"lines\":{count},\"iterations\":{iterations},\"ns_per_op\":{},\"output_len\":{last_len},\"output_capacity\":{capacity},\"borrowed_output\":{borrowed},\"same_pointer\":{reuse}}}",
        total / iterations as u128
    );
    Ok(())
}
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::ptr_arg,
    reason = "The audit measures Cow ownership and String capacity as well as text"
)]
fn print_result(
    variant: &str,
    case: &str,
    kind: &str,
    mode: &str,
    length: usize,
    count: usize,
    iterations: usize,
    elapsed: u128,
    output: &Cow<'_, str>,
    reuse: bool,
) {
    let capacity = match output {
        Cow::Owned(text) => text.capacity(),
        Cow::Borrowed(_) => 0,
    };
    let current_rss = std::fs::read_to_string("/proc/self/smaps_rollup")
        .ok()
        .and_then(|text| {
            text.lines().find_map(|line| {
                line.strip_prefix("Rss:")
                    .and_then(|value| value.split_whitespace().next())
                    .and_then(|value| value.parse::<usize>().ok())
            })
        })
        .map_or("null".to_owned(), |value| value.to_string());
    println!(
        "{{\"variant\":\"{variant}\",\"case\":\"{case}\",\"dataset\":\"{kind}\",\"ownership\":\"{mode}\",\"bytes\":{length},\"lines\":{count},\"iterations\":{iterations},\"current_rss_kib\":{current_rss},\"ns_per_op\":{elapsed},\"output_len\":{},\"output_capacity\":{capacity},\"borrowed_output\":{},\"same_pointer\":{reuse}}}",
        output.len(),
        matches!(output, Cow::Borrowed(_))
    );
}
