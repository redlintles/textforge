use std::borrow::Cow;

use crate::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    utils::{errors::TextForgeError, transforms::get_safe_utf8_char_index},
};

fn process_run(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    debug: bool,
) -> Result<String, TextForgeError> {
    if debug {
        processor.process_all_with_debug(identifier, input)
    } else {
        processor.process_all(identifier, input)
    }
}

pub fn process_input_single_chunk(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    debug: bool,
) -> Result<String, TextForgeError> {
    if input.is_empty() {
        return Ok(String::new());
    }
    process_run(processor, identifier, input, debug)
}

pub fn process_input_line_by_line(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    debug: bool,
) -> Result<String, TextForgeError> {
    if input.is_empty() {
        return Ok(String::new());
    }
    let mut text_vec = input.lines().map(|x| x.to_string()).collect::<Vec<_>>();

    for line in text_vec.iter_mut() {
        *line = process_run(processor, identifier, line, debug)?;
    }

    Ok(text_vec.join("\n"))
}

pub fn process_input_by_chunks(
    processor: &mut TextForgeProcessor,
    identifier: &str,
    input: &str,
    chunk_size: usize,
    debug: bool,
) -> Result<String, TextForgeError> {
    if input.is_empty() {
        return Ok(String::new());
    }
    if chunk_size == 0 {
        return Err(TextForgeError::new(
            super::errors::TextForgeErrorCode::ZeroDivisionError("chunk size == 0".into()),
            Cow::Borrowed("process_input_by_chunks"),
            Cow::Owned(input.to_string()),
        ));
    }
    let character_count = input.chars().count();
    let iterations = character_count.div_ceil(chunk_size);

    let mut processed = Vec::new();

    for i in 0..iterations - 1 {
        let left_chunk_bound = get_safe_utf8_char_index(i * chunk_size, input)?;
        let right_chunk_bound = get_safe_utf8_char_index((i + 1) * chunk_size, input)?;

        let result = process_run(
            processor,
            identifier,
            &input[left_chunk_bound..right_chunk_bound],
            debug,
        )?;

        processed.push(result);
    }

    // Último chunk
    let left_chunk_bound = get_safe_utf8_char_index((iterations - 1) * chunk_size, input)?;
    let result = process_run(processor, identifier, &input[left_chunk_bound..], debug)?;
    processed.push(result);

    Ok(processed.join(""))
}

#[cfg(feature = "test_access")]
#[cfg(test)]
mod cli_tests {
    mod process_input_by_chunks_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_by_chunks, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha";
            let expected_output = "bcolbxilbnhlbal".to_string();
            let result = process_input_by_chunks(&mut processor, &id, input, 2, true)?;

            println!("Resultado: {}", result);

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }

    mod process_input_line_by_line_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_line_by_line, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha\nlaranja";
            let expected_output = "bcoxinhal\nblaranjal".to_string();
            let result = process_input_line_by_line(&mut processor, &id, input, true)?;

            println!("Resultado: {}", result);

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }

    mod process_input_single_chunk_tests {
        use crate::{
            api::{TextForgeBuilderMethods, processor::TextForgeProcessor},
            utils::{cli::process_input_single_chunk, errors::TextForgeError},
        };

        #[test]
        fn it_works_correctly() -> Result<(), TextForgeError> {
            let mut processor = TextForgeProcessor::new();

            let id = processor
                .create_pipeline()
                .add_to_beginning("b")?
                .add_to_end("l")?
                .build()?;

            let input = "coxinha";
            let expected_output = "bcoxinhal".to_string();
            let result = process_input_single_chunk(&mut processor, &id, input, true)?;

            println!("Resultado: {}", result);

            assert_eq!(result, expected_output, "It works correctly");
            Ok(())
        }
    }
}

/// Where a value of the effective profile came from (`txf config show`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileSource {
    /// The `balanced` default.
    Default,
    /// A preset chosen elsewhere (its name).
    Preset(String),
    /// A configuration file.
    File(std::path::PathBuf),
    /// A command-line flag.
    Flag,
}

impl std::fmt::Display for ProfileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileSource::Default => f.write_str("default"),
            ProfileSource::Preset(name) => write!(f, "preset {name}"),
            ProfileSource::File(path) => write!(f, "file {}", path.display()),
            ProfileSource::Flag => f.write_str("command-line flag"),
        }
    }
}

/// Profile-related `txf` flags.
#[derive(Clone, Debug, Default)]
pub struct ProfileOptions {
    /// `--profile`
    pub profile: Option<String>,
    /// `--spool-threshold`
    pub spool_threshold: Option<String>,
    /// `--config`
    pub config: Option<std::path::PathBuf>,
    /// `--no-rc`
    pub no_rc: bool,
}

/// The effective profile and the origin of each value.
#[derive(Clone, Debug)]
pub struct ResolvedProfile {
    pub profile: crate::api::ExecutionProfile,
    pub preset: String,
    pub preset_source: ProfileSource,
    pub spool_threshold_source: ProfileSource,
}

/// Resolve the `txf` profile. For each value, the strongest source wins:
/// command-line flags, then the file given with `--config`, then `.txfrc` in
/// `directory` (unless `--no-rc`), then the `balanced` preset. Only one file
/// is read; parent directories are not searched and files are never merged.
pub fn resolve_profile(
    options: &ProfileOptions,
    directory: &std::path::Path,
) -> Result<ResolvedProfile, TextForgeError> {
    use crate::api::{ExecutionProfile, RcFile, profile::parse_size};

    let file = match (&options.config, options.no_rc) {
        (Some(path), _) => Some(path.clone()),
        (None, false) => Some(directory.join(".txfrc")).filter(|path| path.is_file()),
        (None, true) => None,
    };
    let rc = file.as_deref().map(RcFile::from_path).transpose()?;

    let (preset, preset_source) = match (&options.profile, rc.as_ref().and_then(RcFile::preset)) {
        (Some(name), _) => (name.clone(), ProfileSource::Flag),
        (None, Some(name)) => (
            name.to_owned(),
            ProfileSource::File(file.clone().unwrap_or_default()),
        ),
        (None, None) => ("balanced".to_owned(), ProfileSource::Default),
    };
    let mut profile = ExecutionProfile::preset(&preset)?;
    let mut spool_threshold_source = ProfileSource::Preset(preset.clone());
    if let Some(threshold) = rc
        .as_ref()
        .map(RcFile::spool_threshold)
        .transpose()?
        .flatten()
    {
        profile.spool_threshold = threshold;
        spool_threshold_source = ProfileSource::File(file.clone().unwrap_or_default());
    }
    if let Some(text) = &options.spool_threshold {
        profile.spool_threshold = parse_size(text)?;
        spool_threshold_source = ProfileSource::Flag;
    }
    Ok(ResolvedProfile {
        profile,
        preset,
        preset_source,
        spool_threshold_source,
    })
}

impl ResolvedProfile {
    /// The text `txf config show` prints.
    pub fn describe(&self) -> String {
        let mut text = format!(
            "profile: {} ({})\nspool_threshold: {} bytes ({})\n",
            self.preset,
            self.preset_source,
            self.profile.spool_threshold,
            self.spool_threshold_source
        );
        if !cfg!(feature = "buffered") {
            text.push_str(
                "note: this txf was built without the `buffered` feature; spool_threshold has no effect\n",
            );
        }
        text
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;
    use crate::api::ExecutionProfile;
    use std::path::PathBuf;

    struct Directory(PathBuf);
    impl Directory {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("txf-{name}-{}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Directory(path)
        }
        fn rc(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn without_files_or_flags_the_profile_is_balanced() {
        let directory = Directory::new("none");
        let resolved = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap();
        assert_eq!(resolved.profile, ExecutionProfile::balanced());
        assert_eq!(resolved.preset_source, ProfileSource::Default);
        assert!(
            resolved
                .describe()
                .starts_with("profile: balanced (default)\n")
        );
    }

    #[test]
    fn flags_beat_the_config_file_which_beats_the_local_rc() {
        let directory = Directory::new("precedence");
        let local = directory.rc(
            ".txfrc",
            r#"{"version": 1, "profile": "low-memory", "overrides": {"spool_threshold": "8KiB"}}"#,
        );
        let explicit = directory.rc("other.json", r#"{"version": 1, "profile": "fast"}"#);

        let resolved = resolve_profile(&ProfileOptions::default(), &directory.0).unwrap();
        assert_eq!(resolved.preset, "low-memory");
        assert_eq!(resolved.profile.spool_threshold, 8192);
        assert_eq!(
            resolved.spool_threshold_source,
            ProfileSource::File(local.clone())
        );

        // --config replaces the local file entirely (no merging).
        let options = ProfileOptions {
            config: Some(explicit.clone()),
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.profile, ExecutionProfile::fast());
        assert_eq!(resolved.preset_source, ProfileSource::File(explicit));

        // Flags win, value by value.
        let options = ProfileOptions {
            profile: Some("fast".into()),
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.preset_source, ProfileSource::Flag);
        assert_eq!(resolved.profile.spool_threshold, 8192, "file override kept");
        let options = ProfileOptions {
            spool_threshold: Some("2MiB".into()),
            no_rc: true,
            ..Default::default()
        };
        let resolved = resolve_profile(&options, &directory.0).unwrap();
        assert_eq!(resolved.preset, "balanced");
        assert_eq!(resolved.profile.spool_threshold, 2 * 1024 * 1024);
        assert_eq!(resolved.spool_threshold_source, ProfileSource::Flag);
        assert!(resolved.describe().contains("(command-line flag)"));
    }

    #[test]
    fn invalid_files_and_flags_are_errors() {
        let directory = Directory::new("invalid");
        directory.rc(".txfrc", r#"{"version": 1, "mystery": true}"#);
        assert!(resolve_profile(&ProfileOptions::default(), &directory.0).is_err());
        // --no-rc ignores it.
        let options = ProfileOptions {
            no_rc: true,
            ..Default::default()
        };
        assert!(resolve_profile(&options, &directory.0).is_ok());
        for options in [
            ProfileOptions {
                profile: Some("turbo".into()),
                no_rc: true,
                ..Default::default()
            },
            ProfileOptions {
                spool_threshold: Some("1".into()),
                no_rc: true,
                ..Default::default()
            },
            ProfileOptions {
                config: Some(directory.0.join("missing.json")),
                ..Default::default()
            },
        ] {
            assert!(resolve_profile(&options, &directory.0).is_err());
        }
    }
}
