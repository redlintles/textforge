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
    /// Safety limits for instructions that multiply work (see [`Limits`]).
    pub limits: Limits,
}

/// Limits on instructions whose cost grows with a parameter, so a typo such
/// as `rpt 99999999999;` fails quickly instead of exhausting memory, disk or
/// CPU. Within the limits results are never affected; past them the
/// instruction fails with a `ValidationError` naming the limit. `None` means
/// unlimited. The defaults are generous and the same in every preset; raise or
/// remove them in `.txfrc` (`overrides`) or with the `txf` flags.
///
/// ```
/// use textforge::api::{ExecutionProfile, Limits, TextForgeBuilderMethods};
/// use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
///
/// let mut profile = ExecutionProfile::balanced();
/// profile.limits.max_generated_bytes = Some(8);
/// let mut processor = TextForgeProcessor::new().with_profile(profile);
/// let id = processor.create_pipeline().repeat(3)?.build()?;
/// assert_eq!(processor.process_all(&id, "ab")?, "ababab");
/// assert!(processor.process_all(&id, "abc").is_err()); // 9 bytes > 8
///
/// processor.set_profile(ExecutionProfile { limits: Limits::unlimited(), ..profile });
/// assert_eq!(processor.process_all(&id, "abc")?, "abcabcabc");
/// # Ok::<(), textforge::utils::errors::TextForgeError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Maximum size, in bytes, of the text `rpt`, `padl` and `padr` produce.
    /// Default: 2 GiB.
    pub max_generated_bytes: Option<u64>,
    /// Maximum `iter` count. Default: 10,000,000.
    pub max_iterations: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_generated_bytes: Some(2 * GIB),
            max_iterations: Some(10_000_000),
        }
    }
}

impl Limits {
    /// No limits at all.
    pub fn unlimited() -> Self {
        Limits {
            max_generated_bytes: None,
            max_iterations: None,
        }
    }

    /// Fail unless `bytes` (`None` when it overflows) fits `max_generated_bytes`.
    pub(crate) fn check_generated(
        &self,
        bytes: Option<u128>,
        instruction: &'static str,
    ) -> Result<(), TextForgeError> {
        let Some(limit) = self.max_generated_bytes else {
            return Ok(());
        };
        match bytes {
            Some(bytes) if bytes <= u128::from(limit) => Ok(()),
            _ => Err(exceeded(
                instruction,
                "max_generated_bytes",
                bytes.map_or_else(|| "more than u128".to_owned(), |b| format!("{b} bytes")),
                limit,
            )),
        }
    }

    /// Fail unless `count` fits `max_iterations`.
    pub(crate) fn check_iterations(
        &self,
        count: usize,
        instruction: &'static str,
    ) -> Result<(), TextForgeError> {
        match self.max_iterations {
            Some(limit) if count as u128 > u128::from(limit) => Err(exceeded(
                instruction,
                "max_iterations",
                format!("{count} iterations"),
                limit,
            )),
            _ => Ok(()),
        }
    }
}

fn exceeded(
    instruction: &'static str,
    limit_name: &str,
    needed: String,
    limit: u64,
) -> TextForgeError {
    TextForgeError::new(
        TextForgeErrorCode::ValidationError(
            format!("{instruction} exceeds the {limit_name} limit").into(),
        ),
        instruction,
        format!(
            "needs {needed}, limit {limit}; raise or remove it with overrides.{limit_name} in .txfrc (\"unlimited\"), the txf flag, or ExecutionProfile::limits"
        ),
    )
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
            limits: Limits::default(),
        }
    }

    /// Spool documents to disk early: 64 KiB.
    pub fn low_memory() -> Self {
        ExecutionProfile {
            spool_threshold: (64 * KIB) as usize,
            limits: Limits::default(),
        }
    }

    /// Keep documents in memory up to 256 MiB.
    pub fn fast() -> Self {
        ExecutionProfile {
            spool_threshold: (256 * MIB) as usize,
            limits: Limits::default(),
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

/// Limit overrides in a `.txfrc`: `None` when a limit is not set there,
/// `Some(None)` when it is set to `"unlimited"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LimitOverrides {
    pub max_generated_bytes: Option<Option<u64>>,
    pub max_iterations: Option<Option<u64>>,
}

/// Keys accepted at the top of a `.txfrc` (kept in sync with
/// `docs/txfrc.schema.json` by a test).
pub const RC_KEYS: &[&str] = &["version", "profile", "overrides"];
/// Keys accepted in `overrides`.
pub const RC_OVERRIDE_KEYS: &[&str] = &["spool_threshold", "max_generated_bytes", "max_iterations"];

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Overrides {
    #[serde(default)]
    spool_threshold: Option<Size>,
    #[serde(default)]
    max_generated_bytes: Option<Size>,
    #[serde(default)]
    max_iterations: Option<Size>,
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

/// Parse a size for `spool_threshold`: bytes, or a number followed by `B`,
/// `KiB`, `MiB` or `GiB` (case-insensitive, optional space). Checked against
/// the accepted range.
pub fn parse_size(text: &str) -> Result<usize, TextForgeError> {
    checked_threshold(parse_bytes(text)?)
}

/// Parse a limit: `"unlimited"`, a number, or (for byte limits) a size with a
/// binary unit. Returns `None` for unlimited.
pub fn parse_limit(text: &str) -> Result<Option<u64>, TextForgeError> {
    if text.trim().eq_ignore_ascii_case("unlimited") {
        return Ok(None);
    }
    parse_bytes(text).map(Some)
}

fn parse_bytes(text: &str) -> Result<u64, TextForgeError> {
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
    number
        .parse::<u64>()
        .ok()
        .and_then(|number| number.checked_mul(multiplier))
        .ok_or_else(|| invalid("Invalid size", text))
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
        rc.limits()?;
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

    /// The limit overrides in the file.
    pub fn limits(&self) -> Result<LimitOverrides, TextForgeError> {
        let read = |size: &Option<Size>| -> Result<Option<Option<u64>>, TextForgeError> {
            match size {
                None => Ok(None),
                Some(Size::Bytes(value)) => Ok(Some(Some(*value))),
                Some(Size::Text(text)) => parse_limit(text).map(Some),
            }
        };
        Ok(LimitOverrides {
            max_generated_bytes: read(&self.overrides.max_generated_bytes)?,
            max_iterations: read(&self.overrides.max_iterations)?,
        })
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
        let overrides = self.limits()?;
        if let Some(limit) = overrides.max_generated_bytes {
            profile.limits.max_generated_bytes = limit;
        }
        if let Some(limit) = overrides.max_iterations {
            profile.limits.max_iterations = limit;
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
    fn limits_accept_numbers_sizes_and_unlimited() {
        let rc = RcFile::parse(
            r#"{"version": 1, "overrides": {"max_generated_bytes": "64GiB", "max_iterations": 5}}"#,
        )
        .unwrap();
        let profile = rc.profile().unwrap();
        assert_eq!(profile.limits.max_generated_bytes, Some(64 * GIB));
        assert_eq!(profile.limits.max_iterations, Some(5));
        let rc =
            RcFile::parse(r#"{"version": 1, "overrides": {"max_generated_bytes": "Unlimited"}}"#)
                .unwrap();
        assert_eq!(rc.profile().unwrap().limits.max_generated_bytes, None);
        assert_eq!(
            rc.profile().unwrap().limits.max_iterations,
            Some(10_000_000)
        );
        assert!(
            RcFile::parse(r#"{"version": 1, "overrides": {"max_iterations": "lots"}}"#).is_err()
        );
    }

    #[test]
    fn limits_report_what_was_needed_and_how_to_raise_it() {
        let limits = Limits {
            max_generated_bytes: Some(10),
            max_iterations: Some(3),
        };
        assert!(limits.check_generated(Some(10), "rpt").is_ok());
        let error = limits.check_generated(Some(11), "rpt").unwrap_err();
        assert!(error.input.contains("11 bytes") && error.input.contains("max_generated_bytes"));
        assert!(limits.check_generated(None, "rpt").is_err());
        assert!(limits.check_iterations(3, "iter").is_ok());
        assert!(limits.check_iterations(4, "iter").is_err());
        let unlimited = Limits::unlimited();
        assert!(unlimited.check_generated(None, "rpt").is_ok());
        assert!(unlimited.check_iterations(usize::MAX, "iter").is_ok());
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
            r#"{"version": 1, "profile": "low-memory", "overrides": {"spool_threshold": "1MiB",
                "max_generated_bytes": "16GiB", "max_iterations": "unlimited"}}"#,
        )
        .unwrap();
    }
}
