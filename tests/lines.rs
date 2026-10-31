fn verify_environments(
    name: &str,
    arguments: Vec<textforge::api::portable::Argument>,
    input: &str,
    expected: &str,
) {
    use textforge::api::portable::{Argument, InstructionSpec, PipelinePlan};
    let instruction = InstructionSpec::new(name.into(), arguments.clone());
    let mut dynamic = PipelinePlan::new();
    let mut variables = Vec::new();
    for (i, argument) in arguments.iter().enumerate() {
        let value = match argument {
            Argument::Text(text) => text.clone(),
            Argument::Number(number) => number.to_string(),
            _ => unreachable!(),
        };
        let key = format!("line_parameter_{i}");
        dynamic
            .add(InstructionSpec::new(
                "val".into(),
                vec![Argument::Text(key.clone()), Argument::Text(value)],
            ))
            .unwrap();
        variables.push(Argument::Variable(key));
    }
    dynamic
        .add(InstructionSpec::new(name.into(), variables))
        .unwrap();
    let executable = dynamic.compile().unwrap();
    assert_eq!(executable.process(input).unwrap(), expected);
    assert_eq!(executable.process(input).unwrap(), expected);
    let mut body = PipelinePlan::new();
    body.add(instruction.clone()).unwrap();
    let mut block = PipelinePlan::new();
    block.block("lines".into(), body);
    block.call_block("lines".into()).unwrap();
    assert_eq!(block.compile().unwrap().process(input).unwrap(), expected);
    let mut body = PipelinePlan::new();
    body.add(instruction.clone()).unwrap();
    let mut conditional = PipelinePlan::new();
    conditional.contains_each("".into(), body);
    assert_eq!(
        conditional.compile().unwrap().process(input).unwrap(),
        expected
    );
    #[cfg(feature = "test_access")]
    {
        use textforge::api::portable::{
            BufferOptions, FilePath, Observers, PortableBuffer, PortableProcessor,
        };
        let mut processor = PortableProcessor::new();
        let id = processor.add_transform(vec![instruction]).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let options = BufferOptions {
            memory_threshold: 1,
            spill_directory: FilePath::from_text(directory.path().to_str().unwrap().into()),
        };
        let buffer = PortableBuffer::from_text(input, options).unwrap();
        assert_eq!(
            processor
                .process_all_buffered(&id, buffer)
                .unwrap()
                .into_string()
                .unwrap(),
            expected
        );
        let mut observers = Observers::new();
        observers
            .set_builtin("length".into(), "current_len_chars")
            .unwrap();
        observers.schedule_all("length").unwrap();
        let report = FilePath::from_text(
            directory
                .path()
                .join("report.json")
                .to_str()
                .unwrap()
                .into(),
        );
        assert_eq!(
            processor
                .process_all_with_watchers(&id, input, &mut observers, &report)
                .unwrap(),
            expected
        );
    }
}

#[test]
fn apl_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .append_line("b")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nb");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Text("b".into())];
    verify_environments("apl", arguments.clone(), input, "a\nb");
    plan.add(InstructionSpec::new("apl".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nb");
    use textforge::tokens::{InstructionMethods, transforms::apl::Apl};
    let token = Apl::new("b").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nb"
        );
    }
    assert_eq!(token.get_string_repr(), "apl");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nb");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x42u32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn dln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_line(1)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(1)];
    verify_environments("dln", arguments.clone(), input, "b");
    plan.add(InstructionSpec::new("dln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b");
    use textforge::tokens::{InstructionMethods, transforms::dln::Dln};
    let token = Dln::new(1).unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b"
        );
    }
    assert_eq!(token.get_string_repr(), "dln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x43u32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn dlna_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_lines_after(1)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\n");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(1)];
    verify_environments("dlna", arguments.clone(), input, "a\n");
    plan.add(InstructionSpec::new("dlna".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\n");
    use textforge::tokens::{InstructionMethods, transforms::dlna::Dlna};
    let token = Dlna::new(1).unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\n"
        );
    }
    assert_eq!(token.get_string_repr(), "dlna");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\n");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x44u32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn dlnb_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_lines_before(2)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(2)];
    verify_environments("dlnb", arguments.clone(), input, "b");
    plan.add(InstructionSpec::new("dlnb".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b");
    use textforge::tokens::{InstructionMethods, transforms::dlnb::Dlnb};
    let token = Dlnb::new(2).unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b"
        );
    }
    assert_eq!(token.get_string_repr(), "dlnb");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x45u32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn slnc_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb\nc";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .select_lines(2, 2)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b\n");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(2), Argument::Number(2)];
    verify_environments("slnc", arguments.clone(), input, "b\n");
    plan.add(InstructionSpec::new("slnc".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b\n");
    use textforge::tokens::{InstructionMethods, transforms::slnc::Slnc};
    let token = Slnc::new(2, 2).unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b\n"
        );
    }
    assert_eq!(token.get_string_repr(), "slnc");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b\n");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x46u32.to_be_bytes());
        assert_eq!(bytes[12], 2);
    }
}

#[test]
fn dlni_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb\nc";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_lines_interval(2, 2)
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nc");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(2), Argument::Number(2)];
    verify_environments("dlni", arguments.clone(), input, "a\nc");
    plan.add(InstructionSpec::new("dlni".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nc");
    use textforge::tokens::{InstructionMethods, transforms::dlni::Dlni};
    let token = Dlni::new(2, 2).unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nc"
        );
    }
    assert_eq!(token.get_string_repr(), "dlni");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nc");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x47u32.to_be_bytes());
        assert_eq!(bytes[12], 2);
    }
}

#[test]
fn inl_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .insert_line(2, "x")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nx\nb");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(2), Argument::Text("x".into())];
    verify_environments("inl", arguments.clone(), input, "a\nx\nb");
    plan.add(InstructionSpec::new("inl".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nx\nb");
    use textforge::tokens::{InstructionMethods, transforms::inl::Inl};
    let token = Inl::new(2, "x").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nx\nb"
        );
    }
    assert_eq!(token.get_string_repr(), "inl");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nx\nb");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x48u32.to_be_bytes());
        assert_eq!(bytes[12], 2);
    }
}

#[test]
fn nln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .remove_line_breaks()
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "ab");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![];
    verify_environments("nln", arguments.clone(), input, "ab");
    plan.add(InstructionSpec::new("nln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "ab");
    use textforge::tokens::{InstructionMethods, transforms::nln::Nln};
    let token = Nln::new().unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "ab"
        );
    }
    assert_eq!(token.get_string_repr(), "nln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "ab");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x49u32.to_be_bytes());
        assert_eq!(bytes[12], 0);
    }
}

#[test]
fn rln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .replace_line(2, "x")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nx");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Number(2), Argument::Text("x".into())];
    verify_environments("rln", arguments.clone(), input, "a\nx");
    plan.add(InstructionSpec::new("rln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nx");
    use textforge::tokens::{InstructionMethods, transforms::rln::Rln};
    let token = Rln::new(2, "x").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nx"
        );
    }
    assert_eq!(token.get_string_repr(), "rln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nx");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4au32.to_be_bytes());
        assert_eq!(bytes[12], 2);
    }
}

#[test]
fn pln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .prepend_line("b")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b\na");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Text("b".into())];
    verify_environments("pln", arguments.clone(), input, "b\na");
    plan.add(InstructionSpec::new("pln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b\na");
    use textforge::tokens::{InstructionMethods, transforms::pln::Pln};
    let token = Pln::new("b").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b\na"
        );
    }
    assert_eq!(token.get_string_repr(), "pln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b\na");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4bu32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn dle_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\n\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_empty_lines()
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nb");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![];
    verify_environments("dle", arguments.clone(), input, "a\nb");
    plan.add(InstructionSpec::new("dle".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nb");
    use textforge::tokens::{InstructionMethods, transforms::dle::Dle};
    let token = Dle::new().unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nb"
        );
    }
    assert_eq!(token.get_string_repr(), "dle");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nb");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4cu32.to_be_bytes());
        assert_eq!(bytes[12], 0);
    }
}

#[test]
fn klm_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .keep_lines_matching("^b$")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Text("^b$".into())];
    verify_environments("klm", arguments.clone(), input, "b");
    plan.add(InstructionSpec::new("klm".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b");
    use textforge::tokens::{InstructionMethods, transforms::klm::Klm};
    let token = Klm::new("^b$").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b"
        );
    }
    assert_eq!(token.get_string_repr(), "klm");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4du32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn dlm_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .delete_lines_matching("^b$")
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\n");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![Argument::Text("^b$".into())];
    verify_environments("dlm", arguments.clone(), input, "a\n");
    plan.add(InstructionSpec::new("dlm".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\n");
    use textforge::tokens::{InstructionMethods, transforms::dlm::Dlm};
    let token = Dlm::new("^b$").unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\n"
        );
    }
    assert_eq!(token.get_string_repr(), "dlm");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\n");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4eu32.to_be_bytes());
        assert_eq!(bytes[12], 1);
    }
}

#[test]
fn revln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .reverse_lines()
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "b\na");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![];
    verify_environments("revln", arguments.clone(), input, "b\na");
    plan.add(InstructionSpec::new("revln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "b\na");
    use textforge::tokens::{InstructionMethods, transforms::revln::Revln};
    let token = Revln::new().unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "b\na"
        );
    }
    assert_eq!(token.get_string_repr(), "revln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "b\na");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x4fu32.to_be_bytes());
        assert_eq!(bytes[12], 0);
    }
}

#[test]
fn uniqln_native_portable_text_and_bytecode() {
    use textforge::api::{
        TextForgeBuilderMethods,
        portable::{Argument, InstructionSpec, PipelinePlan},
        processor::{TextForgeProcessor, TextForgeProcessorMethods},
    };
    let input = "a\na\nb";
    let mut native = TextForgeProcessor::new();
    let id = native
        .create_pipeline()
        .unique_lines()
        .unwrap()
        .build()
        .unwrap();
    assert_eq!(native.process_all(&id, input).unwrap(), "a\nb");
    let mut plan = PipelinePlan::new();
    let arguments: Vec<Argument> = vec![];
    verify_environments("uniqln", arguments.clone(), input, "a\nb");
    plan.add(InstructionSpec::new("uniqln".into(), arguments))
        .unwrap();
    assert_eq!(plan.compile().unwrap().process(input).unwrap(), "a\nb");
    use textforge::tokens::{InstructionMethods, transforms::uniqln::Uniqln};
    let token = Uniqln::new().unwrap();
    #[cfg(feature = "test_access")]
    {
        let line = token.to_textforge_line();
        assert_eq!(
            native
                .process_single(
                    textforge::text::reader::read_from_text(&line).unwrap(),
                    input
                )
                .unwrap(),
            "a\nb"
        );
    }
    assert_eq!(token.get_string_repr(), "uniqln");
    #[cfg(feature = "test_access")]
    {
        let dir = tempfile::tempdir().unwrap();
        for extension in ["textforge", "textforgebc"] {
            let path = dir.path().join(format!("pipeline.{extension}"));
            std::fs::write(&path, "").unwrap();
            let restored = if extension == "textforge" {
                native.write_pipeline_to_file(&id, &path).unwrap();
                native.get_pipeline_from_file(&path).unwrap()
            } else {
                native.write_to_bytecode_file(&id, &path).unwrap();
                native.read_from_bytecode_file(&path).unwrap()
            };
            assert_eq!(native.process_all(&restored, input).unwrap(), "a\nb");
        }
    }
    #[cfg(feature = "bytecode")]
    {
        let bytes = token.to_bytecode().unwrap();
        assert_eq!(&bytes[8..12], &0x50u32.to_be_bytes());
        assert_eq!(bytes[12], 0);
    }
}
