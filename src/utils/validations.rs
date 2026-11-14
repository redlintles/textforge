use std::{borrow::Cow, path::Path};

use crate::utils::errors::{TextForgeError, TextForgeErrorCode};

pub fn check_file_path(path: &Path, ext: Option<&str>) -> Result<(), TextForgeError> {
    let allowed_extensions = ["textforge", "tft", "textforgebc", "tfb"];
    let parsed_ext = ext.unwrap_or("tft");

    // canonicalize() exige que o path exista; isso é ok aqui porque queremos validar um arquivo real.
    let path = path.canonicalize().map_err(|e| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("cannot open the pipeline file {}: {e}", path.display()).into(),
            ),
            Cow::Borrowed(""),
            path.display().to_string(),
        )
    })?;

    // Extensão
    let file_ext = path.extension().and_then(|x| x.to_str()).ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                "a pipeline file needs an extension: .textforge or .tft for text, .textforgebc or .tfb for bytecode".into(),
            ),
            Cow::Borrowed(""),
            path.to_string_lossy().to_string(),
        )
    })?;

    if ext.is_some() && file_ext != parsed_ext {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!("expected a `.{parsed_ext}` file, but the extension is `.{file_ext}`")
                    .into(),
            ),
            Cow::Borrowed(""),
            path.to_string_lossy().to_string(),
        ));
    }

    if ext.is_none() && !allowed_extensions.contains(&file_ext) {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                format!(
                    "`.{file_ext}` is not a pipeline extension; use .textforge or .tft for text, .textforgebc or .tfb for bytecode"
                )
                .into(),
            ),
            Cow::Borrowed(""),
            path.to_string_lossy().to_string(),
        ));
    }

    // Diretório pai (redundante depois do canonicalize, mas mantém a intenção explícita)
    let parent = path.parent().ok_or_else(|| {
        TextForgeError::new(
            TextForgeErrorCode::ValidationError("the pipeline path has no parent directory".into()),
            Cow::Borrowed(""),
            path.to_string_lossy().to_string(),
        )
    })?;

    if !parent.exists() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                "the directory of the pipeline file does not exist".into(),
            ),
            Cow::Borrowed(""),
            parent.to_string_lossy().to_string(),
        ));
    }

    if path.is_dir() {
        return Err(TextForgeError::new(
            TextForgeErrorCode::ValidationError(
                "the pipeline path is a directory, not a file".into(),
            ),
            Cow::Borrowed(""),
            path.to_string_lossy().to_string(),
        ));
    }

    Ok(())
}

/// Checks an inclusive character range `start..=end` (character counts, not
/// bytes).
///
/// - `start_index <= end_index` is required (`start == end` selects one item).
/// - With `check_against = Some(text)`, both indices must be within
///   `0..text.chars().count()`.
pub fn check_chunk_bound_indexes(
    start_index: usize,
    end_index: usize,
    check_against: Option<&str>,
) -> Result<(), TextForgeError> {
    let total_chars = check_against.map(|text| text.chars().count());

    // start precisa existir: checado antes da ordem, porque um `end` além do
    // texto é cortado no último caractere e ficaria "antes" de um `start` inexistente
    if let (Some(text), Some(total)) = (check_against, total_chars)
        && start_index >= total
    {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "the range starts at character {start_index}, but the text has {}{}",
                    crate::utils::errors::plural(total, "character"),
                    valid_indices(total)
                )
                .into(),
            ),
            Cow::Borrowed(""),
            crate::utils::errors::input_excerpt(text),
        ));
    }

    // regra estrutural (independente do texto); intervalo inclusivo
    if start_index > end_index {
        return Err(TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(
                format!(
                    "the range start ({start_index}) is after its end ({end_index}); ranges are written start..=end with start <= end"
                )
                .into(),
            ),
            Cow::Borrowed(""),
            format!("start={start_index}, end={end_index}"),
        ));
    }

    // end também precisa existir
    if let (Some(text), Some(total)) = (check_against, total_chars)
        && end_index >= total
    {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "the range ends at character {end_index}, but the text has {}{}",
                    crate::utils::errors::plural(total, "character"),
                    valid_indices(total)
                )
                .into(),
            ),
            Cow::Borrowed(""),
            crate::utils::errors::input_excerpt(text),
        ));
    }

    Ok(())
}

/// `" (valid indices 0 to n-1)"`, or nothing when there is no valid index.
fn valid_indices(count: usize) -> String {
    match count {
        0 => String::new(),
        count => format!(" (valid indices 0 to {})", count - 1),
    }
}

pub fn check_index_against_input(index: usize, input: &str) -> Result<(), TextForgeError> {
    let character_count = input.chars().count();
    if !(0..character_count).contains(&index) {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "character index {index} is out of range: the text has {}{}",
                    crate::utils::errors::plural(character_count, "character"),
                    valid_indices(character_count)
                )
                .into(),
            ),
            Cow::Borrowed(""),
            crate::utils::errors::input_excerpt(input),
        ));
    }

    Ok(())
}

pub fn check_index_against_words(index: usize, input: &str) -> Result<(), TextForgeError> {
    let word_count = input.split_whitespace().count();

    if word_count == 0 {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!("word index {index} is out of range: the text has no words").into(),
            ),
            Cow::Borrowed(""),
            crate::utils::errors::input_excerpt(input),
        ));
    }

    if !(0..word_count).contains(&index) {
        return Err(TextForgeError::new(
            TextForgeErrorCode::IndexOutOfRange(
                format!(
                    "word index {index} is out of range: the text has {} (valid indices 0 to {})",
                    crate::utils::errors::plural(word_count, "word"),
                    word_count - 1
                )
                .into(),
            ),
            Cow::Borrowed(""),
            crate::utils::errors::input_excerpt(input),
        ));
    }

    Ok(())
}

pub fn check_vec_len<T>(
    v: &[T],
    expected_len: usize,
    ctx: impl Into<Cow<'static, str>>,
    payload: impl Into<String>,
) -> Result<(), TextForgeError> {
    if v.len() == expected_len {
        Ok(())
    } else {
        let ctx = ctx.into();
        Err(TextForgeError::new(
            TextForgeErrorCode::InvalidArgumentNumber(
                format!(
                    "`{ctx}` takes {}, but {} given",
                    crate::utils::errors::plural(expected_len, "parameter"),
                    if v.len() == 1 {
                        "1 was".to_owned()
                    } else {
                        format!("{} were", v.len())
                    }
                )
                .into(),
            ),
            ctx,
            payload.into(),
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_tmp_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!("{}_{}_{}", prefix, std::process::id(), nanos));
        p
    }

    #[cfg(test)]
    mod check_vec_len_tests {
        use super::check_vec_len;

        #[test]
        fn check_vec_len_ok_for_string_vec() {
            let v = vec!["a".to_string(), "b".to_string(), "c".to_string()];

            let result = check_vec_len(&v, 3, "check_vec_len", v.join(" "));

            assert!(result.is_ok());
        }

        #[test]
        fn check_vec_len_err_for_string_vec() {
            let v = vec!["a".to_string(), "b".to_string()];

            let result = check_vec_len(&v, 3, "check_vec_len", v.join(" "));

            assert!(result.is_err());

            // Se TextForgeError tiver Display, isso geralmente vira uma checagem bem útil
            let err = result.unwrap_err();
            let msg = format!("{err}");

            // Não assumo a estrutura interna do TextForgeError, então valido por mensagem
            assert!(
                msg.contains("`check_vec_len` takes 3 parameters, but 2 were given"),
                "{msg}"
            );
        }

        #[test]
        fn check_vec_len_is_generic_for_any_type() {
            let v: Vec<u32> = vec![10, 20, 30, 40];

            let ok = check_vec_len(v.as_slice(), 4, "check_vec_len(u32)", "payload-u32");
            assert!(ok.is_ok());

            let err = check_vec_len(v.as_slice(), 3, "check_vec_len(u32)", "payload-u32");
            assert!(err.is_err());
        }

        #[cfg(feature = "bytecode")]
        mod bytecode_tests {
            use super::check_vec_len;
            use crate::parser::params::TextForgeParamTypes;

            #[test]
            fn check_vec_len_ok_for_textforgeparamtypes_vec() {
                let instruction: Vec<TextForgeParamTypes> = vec![
                    TextForgeParamTypes::Usize(1),
                    TextForgeParamTypes::String("abc".to_string()),
                ];

                let result = check_vec_len(
                    instruction.as_slice(),
                    2,
                    "check_vec_len(bytecode)",
                    "<bytecode>",
                );

                assert!(result.is_ok());
            }

            #[test]
            fn check_vec_len_err_for_textforgeparamtypes_vec() {
                let instruction: Vec<TextForgeParamTypes> = vec![
                    TextForgeParamTypes::Usize(1),
                    TextForgeParamTypes::String("abc".to_string()),
                ];

                let result = check_vec_len(
                    instruction.as_slice(),
                    1,
                    "check_vec_len(bytecode)",
                    "<bytecode>",
                );

                assert!(result.is_err());

                let err = result.unwrap_err();
                let msg = format!("{err}");
                assert!(
                    msg.contains("`check_vec_len(bytecode)` takes 1 parameter, but 2 were given"),
                    "{msg}"
                );
            }
        }
    }

    mod check_file_path_tests {
        use super::*;

        #[test]
        fn ok_for_existing_file_with_default_ext_textforge() {
            let dir = unique_tmp_dir("textforge_check_file_path_ok");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.textforge");
            fs::write(&file, "banana").unwrap();

            assert!(check_file_path(&file, None).is_ok());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn ok_for_existing_file_with_custom_ext() {
            let dir = unique_tmp_dir("textforge_check_file_path_custom_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.textforgebc");
            fs::write(&file, "banana").unwrap();

            assert!(check_file_path(&file, None).is_ok());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_wrong_extension() {
            let dir = unique_tmp_dir("textforge_check_file_path_wrong_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input.txt");
            fs::write(&file, "banana").unwrap();

            let r = check_file_path(&file, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_missing_extension() {
            let dir = unique_tmp_dir("textforge_check_file_path_no_ext");
            fs::create_dir_all(&dir).unwrap();

            let file = dir.join("input");
            fs::write(&file, "banana").unwrap();

            let r = check_file_path(&file, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_directory_path() {
            let dir = unique_tmp_dir("textforge_check_file_path_is_dir");
            fs::create_dir_all(&dir).unwrap();

            // canonicalize vai funcionar, mas is_dir deve falhar
            let r = check_file_path(&dir, None);
            assert!(r.is_err());

            let _ = fs::remove_dir_all(&dir);
        }

        #[test]
        fn err_for_nonexistent_path_canonicalize_fails() {
            let dir = unique_tmp_dir("textforge_check_file_path_missing");
            // não cria diretório/arquivo
            let file = dir.join("missing.textforge");

            let r = check_file_path(&file, None);
            assert!(r.is_err());
        }
    }

    mod check_chunk_bound_indexes_tests {
        use super::*;

        #[test]
        fn ok_when_start_lt_end_and_no_text() {
            assert!(check_chunk_bound_indexes(1, 3, None).is_ok());
        }

        #[test]
        fn err_only_when_start_gt_end_no_text() {
            // Inclusive ranges: start == end selects one item (it was an error).
            assert!(check_chunk_bound_indexes(3, 3, None).is_ok());
            assert!(check_chunk_bound_indexes(4, 2, None).is_err());
        }

        #[test]
        fn ok_when_indices_in_range_with_text() {
            // "banàna" tem 6 chars
            let text = "banàna";
            assert!(check_chunk_bound_indexes(1, 4, Some(text)).is_ok());
        }

        #[test]
        fn err_when_start_out_of_range_with_text() {
            let text = "abc"; // 3 chars: idx 0..2
            assert!(check_chunk_bound_indexes(3, 4, Some(text)).is_err());
        }

        #[test]
        fn err_when_end_out_of_range_with_text() {
            let text = "abc"; // 3 chars: idx 0..2
            assert!(check_chunk_bound_indexes(1, 3, Some(text)).is_err());
        }

        #[test]
        fn err_only_when_start_gt_end_with_text() {
            let text = "abcdef";
            assert!(check_chunk_bound_indexes(2, 2, Some(text)).is_ok());
            assert!(check_chunk_bound_indexes(3, 1, Some(text)).is_err());
        }
    }

    mod check_index_against_input_tests {
        use super::*;

        #[test]
        fn ok_when_index_in_range() {
            assert!(check_index_against_input(0, "banana").is_ok());
            assert!(check_index_against_input(5, "banana").is_ok());
        }

        #[test]
        fn err_when_index_out_of_range() {
            assert!(check_index_against_input(6, "banana").is_err());
            assert!(check_index_against_input(999, "banana").is_err());
        }

        #[test]
        fn err_when_input_empty() {
            assert!(check_index_against_input(0, "").is_err());
        }
    }

    mod check_index_against_words_tests {
        use super::*;

        #[test]
        fn ok_when_index_in_range() {
            assert!(check_index_against_words(0, "banana laranja").is_ok());
            assert!(check_index_against_words(1, "banana laranja").is_ok());
        }

        #[test]
        fn ok_with_multiple_spaces_and_newlines() {
            assert!(check_index_against_words(2, "banana   laranja\ncheia\tde  canja").is_ok());
        }

        #[test]
        fn err_when_index_out_of_range() {
            assert!(check_index_against_words(2, "banana laranja").is_err());
            assert!(check_index_against_words(999, "banana laranja").is_err());
        }

        #[test]
        fn err_when_input_has_no_words() {
            assert!(check_index_against_words(0, "").is_err());
            assert!(check_index_against_words(0, "     \n\t  ").is_err());
        }

        #[test]
        fn supports_unicode_words() {
            // split_whitespace funciona bem com unicode; aqui garante que o contador não quebra
            assert!(check_index_against_words(1, "banàna maçã").is_ok());
            assert!(check_index_against_words(2, "banàna maçã").is_err());
        }
    }
}
