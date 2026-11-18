#![cfg(feature = "test_access")]

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::context::execution_context::{GlobalContextMethods, GlobalExecutionContext};
    use crate::parser::resolve_var::TokenWrapper;
    use crate::text::reader::read_from_text;
    use crate::tokens::InstructionMethods;
    use crate::tokens::instructions::tryi::Tryi;
    use crate::tokens::transforms::{dls::Dls, tua::Tua};

    fn wrap(token: impl InstructionMethods + 'static) -> TokenWrapper {
        (Box::new(token) as Box<dyn InstructionMethods>).into()
    }

    #[test]
    fn a_successful_instruction_runs_as_without_tryi() {
        let token = Tryi::new(wrap(Tua::default()));
        assert_eq!(token.transform("ação 🍌".into(), None).unwrap(), "AÇÃO 🍌");
    }

    #[test]
    fn a_failed_instruction_returns_the_text_it_received() {
        let token = Tryi::new(wrap(Dls::new(9)));
        for text in ["", "abc", "é🍌e\u{301}", "a\r\nb"] {
            assert_eq!(token.transform(text.into(), None).unwrap(), text);
        }
    }

    #[test]
    fn a_failure_returns_the_borrowed_text_without_copying_it() {
        let token = Tryi::new(wrap(Dls::new(9)));
        let text = "abc";
        let result = token.transform(Cow::Borrowed(text), None).unwrap();
        assert!(matches!(result, Cow::Borrowed(result) if std::ptr::eq(result, text)));
    }

    #[test]
    fn an_owned_text_that_does_not_change_is_given_back_as_it_is() {
        let token = Tryi::new(wrap(Dls::new(9)));
        let text = String::from("abc");
        let address = text.as_ptr();
        let result = token.transform(Cow::Owned(text), None).unwrap();
        assert!(matches!(&result, Cow::Owned(result) if result.as_ptr() == address));
    }

    #[test]
    fn a_failed_block_puts_back_variables_and_the_block() {
        let mut context = GlobalExecutionContext::new();
        for line in [
            "var total = 0;",
            "blk calc assoc mutv total = 10;",
            "blk calc assoc dls 9;",
        ] {
            read_from_text(line)
                .unwrap()
                .apply_token("x".into(), &mut context)
                .unwrap();
        }
        let token = read_from_text("tryi cblk calc;").unwrap();
        assert_eq!(token.apply_token("ab".into(), &mut context).unwrap(), "ab");
        let total = read_from_text("ate {{total}};").unwrap();
        assert_eq!(total.apply_token("".into(), &mut context).unwrap(), "0");
        // Before the snapshot, a failed cblk lost its block.
        assert!(context.take_block("calc").is_ok());
    }

    #[test]
    fn a_snapshot_copies_nothing_until_the_context_changes() {
        use crate::context::execution_context::{VarEntry, VarValues};
        let mut context = GlobalExecutionContext::new();
        for index in 0..1000 {
            let entry = VarEntry {
                value: VarValues::Usize(index),
                mutable: true,
            };
            context.add_var(&format!("v{index}"), entry).unwrap();
        }
        // Nothing changed: nothing to compare or report.
        let snapshot = context.snapshot();
        assert!(context.restore(snapshot).is_empty());
        let snapshot = context.snapshot();
        context.get_mut_var("v7").unwrap().value = VarValues::Usize(70);
        let entry = VarEntry {
            value: VarValues::Usize(1),
            mutable: true,
        };
        context.add_var("new", entry).unwrap();
        assert_eq!(context.restore(snapshot), vec!["new", "v7"]);
        assert!(context.get_var("new").is_err());
        assert!(matches!(
            context.get_var("v7").unwrap().value,
            VarValues::Usize(7)
        ));
    }

    #[test]
    fn the_inner_instruction_is_parsed_and_written_back() {
        let token = read_from_text("tryi dlc 9 9;").unwrap();
        assert_eq!(token.get_string_repr(), "tryi");
        assert_eq!(
            token.to_text_line_unresolved().unwrap().trim_end(),
            "tryi dlc 9 9;"
        );
    }

    #[test]
    fn rmapd_inside_tryi_is_rejected_like_inside_iter() {
        for line in ["tryi rmapd m add a b;", "iter 2 times rmapd m add a b;"] {
            let token = read_from_text(line).unwrap();
            assert!(
                crate::parser::structure::validate(&token).is_err(),
                "{line}"
            );
        }
    }

    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;
        use crate::parser::params::TextForgeParamTypes;

        #[test]
        fn to_bytecode_has_the_opcode_and_one_instruction() {
            let token = Tryi::new(wrap(Dls::new(9)));
            assert_eq!(token.get_opcode(), 0x5a);
            let bytecode = token.to_bytecode().unwrap();
            assert!(bytecode.len() > 16);
            assert_eq!(&bytecode[8..12], &0x5au32.to_be_bytes());
        }

        #[test]
        fn bytecode_round_trip_keeps_the_inner_instruction() {
            use crate::bytecode::{
                reader::read_bytecode_from_file, writer::write_bytecode_to_file,
            };
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("tryi.textforgebc");
            let token = read_from_text("tryi dls 9;").unwrap();
            write_bytecode_to_file(&path, vec![token]).unwrap();
            let tokens = read_bytecode_from_file(&path).unwrap();
            assert_eq!(tokens[0].get_string_repr(), "tryi");
            match &tokens[0].get_params()[0] {
                crate::parser::resolve_var::ValType::Literal(TextForgeParamTypes::Token(inner)) => {
                    assert_eq!(inner.get_string_repr(), "dls")
                }
                _ => panic!("expected a nested instruction"),
            }
            let mut context = GlobalExecutionContext::new();
            assert_eq!(
                tokens[0].apply_token("abc".into(), &mut context).unwrap(),
                "abc"
            );
        }
    }
}
