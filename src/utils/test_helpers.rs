use crate::api::{TextForgeBlockMethods, TextForgeConditionalMethods};
use crate::api::{TextForgeBuilderMethods, processor::TextForgeProcessor};
use crate::tokens::{
    InstructionMethods,
    transforms::{tbs::Tbs, tua::Tua, uniqln::Uniqln},
};
use crate::utils::errors::TextForgeError;

pub fn build_all_tokens_pipeline_safe(
    processor: &mut TextForgeProcessor,
) -> Result<String, TextForgeError> {
    // File instructions need a readable file and a writable directory.
    let files =
        std::env::temp_dir().join(format!("textforge-safe-pipeline-{}", std::process::id()));
    let io = |error: std::io::Error| {
        TextForgeError::new(
            crate::utils::errors::TextForgeErrorCode::FileWritingError(error.to_string().into()),
            "build_all_tokens_pipeline_safe",
            files.display().to_string(),
        )
    };
    std::fs::create_dir_all(&files).map_err(io)?;
    let piece = files.join("piece.txt");
    std::fs::write(&piece, " FILE é🍌 ").map_err(io)?;
    let piece = piece.to_string_lossy().into_owned();
    let snapshot = files.join("snapshot.txt").to_string_lossy().into_owned();
    let joined = files.join("joined.textforge");
    std::fs::write(&joined, "tua;\nvar inner = 1;\nate ' JOINED';\n").map_err(io)?;
    let joined = joined.to_string_lossy().into_owned();

    let id = processor
        .create_pipeline()
        .normalize_unicode("nfc")?
        .add_replace_rule("safe", "NEVER_MATCH_SENTINEL", "replacement")?
        .execute_replace_map("safe")?
        // ========== PHASE 0: a large, predictable base ==========
        // Ensures repeated Bs and plenty of length from the start.
        .add_to_beginning("B__SEED__B__SEED__B__SEED__ ")?
        // makes split_select("B", 1) safe
        .add_to_end(" Banana Laranja cheia de canja ")?
        // original text
        .pad_right("x", 160)?
        // high length for the indices
        // ========== BLOCK 1: deletes/replace/insert/rotates/trims ==========
        .repeat(3)?
        .delete_after(120)?
        .delete_before(3)?
        .delete_chunk(0, 3)?
        // still a "normal" string here, and the instruction exists
        .delete_first()?
        .delete_last()?
        .replace_all_with(r"a", "e")?
        .replace_first_with("L", "coxinha")?
        .replace_count_with("e", "carro", 3)?
        .insert(0, "Coxinha Banana")?
        .rotate_left(1)?
        .rotate_right(2)?
        .trim_both_sides()?
        .trim_left_side()?
        .trim_right_side()?
        // Checkpoint: after shrinking or changing it, restore the length
        .add_to_end(" B__GUARD__B__GUARD__B__GUARD__ ")?
        // keeps the Bs
        .pad_right("p", 140)?
        // ========== BLOCK 2: select + caps + split_select ==========
        .add_to_beginning("laranjadebananavermelha")?
        .select(0, 60)?
        // a larger, predictable window
        .replace_count_with("a", "b", 3)?
        .to_uppercase_all()?
        .to_lowercase_all()?
        .to_uppercase_single(3)?
        .to_lowercase_single(2)?
        .capitalize_first_word()?
        .capitalize_single_word(1)?
        .capitalize_last_word()?
        .capitalize_range(1, 3)?
        // split_select is safe now, because several "B"s are forced at the start and end
        .split_select("B", 1)?
        // must not become a lowercase "nj" here
        // Checkpoint after split_select (it can shrink the text a lot)
        .add_to_end(" SAFE_SEGMENT_ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789 ")?
        .pad_right("q", 120)?
        // ========== BLOCK 3: replace nth/last + url/html/json ==========
        .capitalize_chunk(1, 3)?
        // still exists, but no chunk has been created yet
        .replace_last_with("b", "c")?
        .replace_nth_with("b", "d", 1)?
        // a smaller nth, so it does not depend on many occurrences
        .to_url_encoded()?
        .to_url_decoded()?
        .to_reverse()?
        .split_characters()?
        // from here on the text is in chunks
        .to_html_escaped()?
        .to_html_unescaped()?
        .to_json_escaped()?
        .to_json_unescaped()?
        // Checkpoint: make sure there are enough chunks (split_characters creates many)
        .insert(1, "banana")?
        // inserts an item at a low position
        .to_uppercase_chunk(0, 6)?
        // short, safe ranges
        .to_lowercase_chunk(0, 10)?
        // ========== BLOCK 4: joins + pads ==========
        .join_to_camel_case()?
        .join_to_kebab_case()?
        .join_to_pascal_case()?
        .join_to_snake_case()?
        .pad_left("xy", 60)?
        .pad_right("yx", 80)?
        // ========== BLOCK 5: if_do_contains_each ==========
        // Always true, because pad_left added "xy" and pad_right added "yx"
        .if_do_contains_each("xy", |b| {
            b.add_to_beginning("Banana")?
                .add_to_end("Bonanza")?
                .capitalize_first_word()?
                .capitalize_last_word()?
                .delete_first()?;
            Ok(())
        })?
        .block_assoc("x", |b| {
            b.add_to_beginning("laranja")?
                .delete_first()?
                .replace_all_with("anja", "anjo")?
                .to_uppercase_chunk(0, 40)?
                .if_do_contains_each("Q", |cb| {
                    cb.replace_all_with("_", "!!!!!!!!!")?;
                    Ok(())
                })?;

            Ok(())
        })?
        .call_block("x")?
        .call_block("x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .append_line("b")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_line(1)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_after(1)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_before(2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .select_lines(2, 2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_interval(2, 2)?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .insert_line(2, "x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .remove_line_breaks()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .replace_line(2, "x")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .prepend_line("b")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_empty_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .keep_lines_matching("^b$")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .delete_lines_matching("^b$")?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .reverse_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .unique_lines()?
        .add_to_end("\nLINE\nLINE\nTAIL")?
        .join_lines(" ")?
        // ========== on*: one instruction on a section ==========
        .add_to_end("\n  LINE  \nLINE\nLINE\nTAIL")?
        .on_line(2, Box::new(Tbs::default()) as Box<dyn InstructionMethods>)?
        .on_line_interval(
            2,
            4,
            Box::new(Uniqln::default()) as Box<dyn InstructionMethods>,
        )?
        .on_chunk(
            0,
            2,
            Box::new(Tua::default()) as Box<dyn InstructionMethods>,
        )?
        // ========== files ==========
        .write_to_file(snapshot.as_str())?
        .append_file(piece.as_str())?
        .insert_file(piece.as_str(), 3)?
        .pipeline_join(joined.as_str())?
        .build()?;

    Ok(id)
}
