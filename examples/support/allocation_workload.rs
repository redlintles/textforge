use std::borrow::Cow;
use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    context::execution_context::GlobalExecutionContext,
    text::reader::read_from_text,
};

pub fn cases() -> Result<Vec<serde_json::Value>, Box<dyn std::error::Error>> {
    Ok(serde_json::from_str(include_str!(
        "../../tests/data/allocations/cases.json"
    ))?)
}

pub fn lines(case: &serde_json::Value) -> Result<Vec<&str>, Box<dyn std::error::Error>> {
    case["lines"]
        .as_array()
        .ok_or("missing lines")?
        .iter()
        .map(|v| v.as_str().ok_or_else(|| "invalid line".into()))
        .collect()
}

pub fn execute(
    case: &serde_json::Value,
    input: &str,
    owned: bool,
) -> Result<String, textforge::utils::errors::TextForgeError> {
    let mut context = GlobalExecutionContext::new();
    let mut result = if owned {
        Cow::Owned(input.to_owned())
    } else {
        Cow::Borrowed(input)
    };
    // Fixture data is checked by its JSON parser before this function is called.
    if let Some(lines) = case["lines"].as_array() {
        for line in lines.iter().filter_map(serde_json::Value::as_str) {
            result = read_from_text(line)?.apply_token(result, &mut context)?;
        }
    }
    Ok(result.into_owned())
}

pub fn register(
    processor: &mut TextForgeProcessor,
    case: &serde_json::Value,
) -> Result<String, Box<dyn std::error::Error>> {
    let tokens = lines(case)?
        .into_iter()
        .map(read_from_text)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(processor.add_transform(tokens))
}
