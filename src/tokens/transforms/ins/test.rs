// src/tokens/transforms/ins/test.rs

#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::InstructionMethods;
    use crate::tokens::transforms::ins::Ins;
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn out_of_range_index_on_empty_input_is_an_error_not_a_panic() {
        // Building the error message used to compute `0 - 1` and overflow.
        let t = Ins::new(1, "x");
        let error = t.transform("".into(), None).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::IndexOutOfRange(_)
        ));
    }

    #[test]
    fn params_sets_fields_and_to_textforge_line_formats() {
        let t = Ins::new(2, "laranja");
        assert_eq!(t.to_textforge_line().as_ref(), "ins 2 laranja;\n");
        assert_eq!(t.get_string_repr(), "ins");
    }

    #[test]
    fn transform_inserts_after_index_like_doc_example() {
        let t = Ins::new(2, "laranja");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "banlaranjaana"
        );
    }

    #[test]
    fn transform_index_zero_inserts_at_the_start() {
        // index 0 -> before 'b' (up to 0.3.1: after it)
        let t = Ins::new(0, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Xbanana"
        );
    }

    #[test]
    fn transform_index_last_char_appends_after_last_char() {
        // chars: b a n a n a (len 6), last index = 5
        // uses nth(index+1) -> None -> split_at(len) => append
        let t = Ins::new(5, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananaX"
        );
    }

    #[test]
    fn transform_index_equal_char_count_appends_current_behavior() {
        // the code fails only when index > chars_count, so index == chars_count is accepted.
        // byte_index uses nth(index+1) => None -> append.
        let t = Ins::new(6, "X"); // chars_count("banana") = 6
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananaX"
        );
    }

    #[test]
    fn transform_unicode_safe_insertion_boundary() {
        // input: "ábc" (chars: á b c)
        // index 0 => at the start; index 1 => after 'b', never inside 'á'
        let t = Ins::new(0, "X");
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("ábc".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "Xábc"
        );
    }

    #[test]
    fn transform_errors_when_index_too_large() {
        let t = Ins::new(999, "X");
        let mut ctx = GlobalExecutionContext::new();

        let got = t.transform("abc".into(), Some(&mut ctx));
        assert!(got.is_err());

        // checks the error kind, not the text
        if let Err(e) = got {
            let _ = e;
        }
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Ins::default();
        let params = vec![TextForgeParamTypes::Usize(1)];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_accepts_usize_and_string() {
        let mut t = Ins::default();
        let params = vec![
            TextForgeParamTypes::Usize(3),
            TextForgeParamTypes::String("laranja".to_string()),
        ];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.to_textforge_line().as_ref(), "ins 3 laranja;\n");
    }

    #[test]
    fn set_params_rejects_wrong_param_types() {
        let mut t = Ins::default();
        let params = vec![
            TextForgeParamTypes::String("x".to_string()),
            TextForgeParamTypes::Usize(3),
        ];

        let got = t.set_params(&params);

        // the first parse_args! fails with "parameter 1 (`index`) must be a non-negative integer, but it is text"
        let expected = Err(crate::utils::errors::TextForgeError::new(
            TextForgeErrorCode::InvalidParameters(
                "parameter 1 (`index`) must be a non-negative integer, but it is text".into(),
            ),
            "",
            "",
        ));

        assert_eq!(got, expected);
    }

    // ============================
    // Bytecode-only tests
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn get_opcode_is_28() {
            let t = Ins::default();
            assert_eq!(t.get_opcode(), 0x28);
        }

        #[test]
        fn to_bytecode_has_expected_header_and_decodes_two_params() {
            let t = Ins::new(7, "laranja");
            let bc = t.to_bytecode().unwrap();

            // minimum header: 8 + 4 + 1 = 13
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x28);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 2);

            // param 1 (Usize)
            let p1_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p1_start = i;
            let p1_end = p1_start + (p1_total - 8);
            let p1_payload = bc[p1_start..p1_end].to_vec();

            let decoded1 = TextForgeParamTypes::from_bytecode(p1_payload).unwrap();
            match decoded1 {
                TextForgeParamTypes::Usize(n) => assert_eq!(n, 7),
                _ => panic!("Expected Usize param #1"),
            }

            // param 2 (String)
            i = p1_end;
            let p2_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap()) as usize;
            i += 8;
            let p2_start = i;
            let p2_end = p2_start + (p2_total - 8);
            let p2_payload = bc[p2_start..p2_end].to_vec();

            let decoded2 = TextForgeParamTypes::from_bytecode(p2_payload).unwrap();
            match decoded2 {
                TextForgeParamTypes::String(s) => assert_eq!(s, "laranja"),
                _ => panic!("Expected String param #2"),
            }
        }
    }
}
