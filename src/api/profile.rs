//! Execution profiles: how much memory the engine may use, never what a
//! pipeline produces (see `docs/execution-profiles.md`).
//!
//! A profile only changes resources. The same pipeline, text and static
//! context give byte-identical output under every profile; the test suite
//! checks this. Pipelines cannot read the profile.
use std::path::Path;

use serde::Deserialize;

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// Engine resources for processing.
///
/// `spool_threshold` is the size, in bytes, above which buffered processing
/// (the `buffered` feature) keeps a document in a temporary file instead of
/// memory. Without `buffered` it has no effect. Preset names are stable;
/// individual fields may change between minor versions.
///
/// ```
/// use textforge::api::{ExecutionProfile, TextForgeBuilderMethods};
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let mut processor = TextForgeProcessor::new().with_profile(ExecutionProfile::low_memory());
/// let id = processor.create_pipeline().trim_both_sides()?.build()?;
/// assert_eq!(processor.process_all(&id, "  same output  ")?, "same output");
/// assert_eq!(processor.profile(), &ExecutionProfile::low_memory());
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionProfile {
    pub spool_threshold: usize,
}

impl Default for ExecutionProfile {
    fn default() -> Self {
        Self::balanced()
    }
}

impl ExecutionProfile {
    /// Smallest accepted `spool_threshold` (4 KiB).
    pub const MIN_SPOOL_THRESHOLD: u64 = 4 * KIB;
    /// Largest accepted `spool_threshold` (64 GiB, or the platform limit).
    pub const MAX_SPOOL_THRESHOLD: u64 = 64 * GIB;

    /// The default, and the behavior before profiles existed: 1 MiB.
    pub fn balanced() -> Self {
        ExecutionProfile {
            spool_threshold: MIB as usize,
        }
    }

    /// Spool documents to disk early: 64 KiB.
    pub fn low_memory() -> Self {
        ExecutionProfile {
            spool_threshold: (64 * KIB) as usize,
        }
    }

    /// Keep documents in memory up to 256 MiB.
    pub fn fast() -> Self {
        ExecutionProfile {
            spool_threshold: (256 * MIB) as usize,
        }
    }

    /// The preset called `name`: `balanced`, `low-memory` or `fast`.
    pub fn preset(name: &str) -> Result<Self, TextForgeError> {
        match name {
            "balanced" => Ok(Self::balanced()),
            "low-memory" => Ok(Self::low_memory()),
            "fast" => Ok(Self::fast()),
            other => Err(invalid(
                "Unknown profile; expected balanced, low-memory or fast",
                other,
            )),
        }
    }

    /// Read a `.txfrc` file (see [`RcFile`]) and apply it to its preset.
    /// Nothing reads configuration files implicitly: only callers of this
    /// function (such as the `txf` CLI) do.
    pub fn from_rc_file(path: &Path) -> Result<Self, TextForgeError> {
        RcFile::from_path(path)?.profile()
    }
}

/// A `.txfrc` configuration file: JSON with a required `version` (1), an
/// optional `profile` preset and optional `overrides`. Unknown keys are errors.
/// It may only change resources, never results.
///
/// ```
/// use textforge::api::{ExecutionProfile, RcFile};
///
/// let rc = RcFile::parse(r#"{"version": 1, "profile": "fast",
///     "overrides": {"spool_threshold": "512MiB"}}"#)?;
/// assert_eq!(rc.preset(), Some("fast"));
/// assert_eq!(rc.profile()?.spool_threshold, 512 * 1024 * 1024);
/// assert!(RcFile::parse(r#"{"version": 1, "separator": ","}"#).is_err());
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RcFile {
    version: u32,
    #[serde(default)]
    profile: Option<String>,
    #[serde(default)]
    overrides: Overrides,
}

/// Keys accepted at the top of a `.txfrc` (kept in sync with
/// `docs/txfrc.schema.json` by a test).
pub const RC_KEYS: &[&str] = &["version", "profile", "overrides"];
/// Keys accepted in `overrides`.
pub const RC_OVERRIDE_KEYS: &[&str] = &["spool_threshold"];

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Overrides {
    #[serde(default)]
    spool_threshold: Option<Size>,
}

/// A size in bytes, as a number or as text with a binary unit (`"512MiB"`).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Size {
    Bytes(u64),
    Text(String),
}

fn invalid(message: &str, detail: impl Into<String>) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::ValidationError(message.to_owned().into()),
        "txfrc",
        detail.into(),
    )
}

/// Parse a size: bytes, or a number followed by `B`, `KiB`, `MiB` or `GiB`
/// (case-insensitive, optional space). Checked against the accepted range.
pub fn parse_size(text: &str) -> Result<usize, TextForgeError> {
    let text = text.trim();
    let split = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    let (number, unit) = text.split_at(split);
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "kib" => KIB,
        "mib" => MIB,
        "gib" => GIB,
        _ => return Err(invalid("Unknown size unit; use B, KiB, MiB or GiB", text)),
    };
    let bytes = number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| invalid("Invalid size", text))?;
    checked_threshold(bytes)
}

fn checked_threshold(bytes: u64) -> Result<usize, TextForgeError> {
    let maximum = ExecutionProfile::MAX_SPOOL_THRESHOLD.min(usize::MAX as u64);
    if !(ExecutionProfile::MIN_SPOOL_THRESHOLD..=maximum).contains(&bytes) {
        return Err(invalid(
            "spool_threshold out of range",
            format!(
                "{bytes} bytes; accepted: {} to {maximum} bytes (4 KiB to 64 GiB)",
                ExecutionProfile::MIN_SPOOL_THRESHOLD
            ),
        ));
    }
    usize::try_from(bytes).map_err(|_| invalid("spool_threshold out of range", bytes.to_string()))
}

impl RcFile {
    /// Parse and validate the JSON text of a `.txfrc`.
    pub fn parse(text: &str) -> Result<Self, TextForgeError> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|error| invalid("Invalid .txfrc", error.to_string()))?;
        // serde would also accept a JSON array in field order.
        if !value.is_object() {
            return Err(invalid("A .txfrc must be a JSON object", ""));
        }
        let rc: RcFile = serde_json::from_value(value)
            .map_err(|error| invalid("Invalid .txfrc", error.to_string()))?;
        if rc.version != 1 {
            return Err(invalid(
                "Unsupported .txfrc version; expected 1",
                rc.version.to_string(),
            ));
        }
        if let Some(name) = &rc.profile {
            ExecutionProfile::preset(name)?;
        }
        rc.spool_threshold()?;
        Ok(rc)
    }

    /// Read and parse a `.txfrc` file.
    pub fn from_path(path: &Path) -> Result<Self, TextForgeError> {
        let text = std::fs::read_to_string(path).map_err(|error| {
            TextForgeError::new(
                TextForgeErrorCode::FileReadingError(error.to_string().into()),
                "txfrc",
                path.display().to_string(),
            )
        })?;
        Self::parse(&text).map_err(|mut error| {
            error.input = format!("{}: {}", path.display(), error.input).into();
            error
        })
    }

    /// The preset named in the file, if any.
    pub fn preset(&self) -> Option<&str> {
        self.profile.as_deref()
    }

    /// The `spool_threshold` override, if any.
    pub fn spool_threshold(&self) -> Result<Option<usize>, TextForgeError> {
        match &self.overrides.spool_threshold {
            None => Ok(None),
            Some(Size::Bytes(bytes)) => checked_threshold(*bytes).map(Some),
            Some(Size::Text(text)) => parse_size(text).map(Some),
        }
    }

    /// The profile this file describes: its preset (or `balanced`) with its
    /// overrides applied.
    pub fn profile(&self) -> Result<ExecutionProfile, TextForgeError> {
        let mut profile = match &self.profile {
            Some(name) => ExecutionProfile::preset(name)?,
            None => ExecutionProfile::balanced(),
        };
        if let Some(threshold) = self.spool_threshold()? {
            profile.spool_threshold = threshold;
        }
        Ok(profile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_accept_bytes_and_binary_units_within_range() {
        assert_eq!(parse_size("4096").unwrap(), 4096);
        assert_eq!(parse_size("64 KiB").unwrap(), 65_536);
        assert_eq!(parse_size("512mib").unwrap(), 512 * 1024 * 1024);
        assert_eq!(parse_size("2GiB").unwrap(), 2 * 1024 * 1024 * 1024);
        for wrong in [
            "4095",
            "65GiB",
            "1 MB",
            "",
            "MiB",
            "-1",
            "1.5MiB",
            "99999999999999999999",
        ] {
            assert!(parse_size(wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn rc_files_are_validated_strictly() {
        let rc = RcFile::parse(r#"{"version": 1}"#).unwrap();
        assert_eq!(rc.profile().unwrap(), ExecutionProfile::balanced());
        let rc =
            RcFile::parse(r#"{"version": 1, "overrides": {"spool_threshold": 8192}}"#).unwrap();
        assert_eq!(rc.profile().unwrap().spool_threshold, 8192);
        for wrong in [
            r#"{}"#,
            r#"{"profile": "fast"}"#,
            r#"{"version": 2}"#,
            r#"{"version": 1, "profile": "turbo"}"#,
            r#"{"version": 1, "unorm": "nfc"}"#,
            r#"{"version": 1, "overrides": {"separator": ","}}"#,
            r#"{"version": 1, "overrides": {"spool_threshold": 10}}"#,
            r#"{"version": 1, "overrides": {"spool_threshold": "big"}}"#,
            r#"[1]"#,
            "not json",
        ] {
            let error = RcFile::parse(wrong).unwrap_err();
            assert!(
                matches!(error.error_code, TextForgeErrorCode::ValidationError(_)),
                "{wrong}"
            );
        }
    }

    #[test]
    fn presets_are_ordered_by_memory_and_balanced_is_the_previous_default() {
        assert_eq!(ExecutionProfile::default(), ExecutionProfile::balanced());
        assert_eq!(ExecutionProfile::balanced().spool_threshold, 1024 * 1024);
        assert!(
            ExecutionProfile::low_memory().spool_threshold
                < ExecutionProfile::balanced().spool_threshold
        );
        assert!(
            ExecutionProfile::balanced().spool_threshold < ExecutionProfile::fast().spool_threshold
        );
        for name in ["balanced", "low-memory", "fast"] {
            assert!(ExecutionProfile::preset(name).is_ok());
        }
    }

    #[test]
    fn the_schema_lists_exactly_the_accepted_keys() {
        let schema: serde_json::Value =
            serde_json::from_str(include_str!("../../docs/txfrc.schema.json")).unwrap();
        let keys = |value: &serde_json::Value| {
            let mut keys: Vec<String> = value["properties"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            keys.sort();
            keys
        };
        let sorted = |list: &[&str]| {
            let mut list: Vec<String> = list.iter().map(|key| key.to_string()).collect();
            list.sort();
            list
        };
        assert_eq!(keys(&schema), sorted(RC_KEYS));
        assert_eq!(
            keys(&schema["properties"]["overrides"]),
            sorted(RC_OVERRIDE_KEYS)
        );
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(
            schema["properties"]["overrides"]["additionalProperties"],
            false
        );
        assert_eq!(schema["required"], serde_json::json!(["version"]));
        let presets: Vec<_> = schema["properties"]["profile"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|name| name.as_str().unwrap())
            .collect();
        for name in &presets {
            assert!(ExecutionProfile::preset(name).is_ok());
        }
        assert_eq!(presets.len(), 3);
        // Every documented key is accepted, together.
        RcFile::parse(
            r#"{"version": 1, "profile": "low-memory", "overrides": {"spool_threshold": "1MiB"}}"#,
        )
        .unwrap();
    }
}
