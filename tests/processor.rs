#[cfg(feature = "test_access")]
#[cfg(test)]
pub mod processor {
    use std::path::Path;

    use textforge::{
        api::{
            TextForgeBuilderMethods,
            processor::{TextForgeProcessor, TextForgeProcessorMethods},
        },
        parser::resolve_var::TokenWrapper,
        tokens::transforms::{atb::Atb, ate::Ate, rpt::Rpt},
        utils::errors::TextForgeError,
    };
    use uuid::Uuid;

    #[test]
    fn test_process_all() -> Result<(), TextForgeError> {
        let mut processor = TextForgeProcessor::new();
        let identifier = processor
            .create_pipeline()
            .add_to_beginning("Banana")?
            .add_to_end("Laranja")?
            .repeat(3 as usize)?
            .build()
            .unwrap();
        let input = "Carimbo verde de deus";

        let output = processor.process_all(&identifier, input)?;

        let expected_output = "BananaCarimbo verde de deusLaranjaBananaCarimbo verde de deusLaranjaBananaCarimbo verde de deusLaranja";

        assert_eq!(output, expected_output, "Unexpected output in process_all");

        Ok(())
    }
    #[test]
    fn test_process_all_with_debug() -> Result<(), TextForgeError> {
        let mut processor = TextForgeProcessor::new();
        let identifier = processor
            .create_pipeline()
            .add_to_beginning("Banana")?
            .add_to_end("Laranja")?
            .repeat(3 as usize)?
            .build()
            .unwrap();
        let input = "Carimbo verde de deus";

        let output = processor.process_all_with_debug(&identifier, input)?;

        let expected_output = "BananaCarimbo verde de deusLaranjaBananaCarimbo verde de deusLaranjaBananaCarimbo verde de deusLaranja";

        assert_eq!(output, expected_output, "Unexpected output in process_all");

        Ok(())
    }

    #[test]
    fn test_process_single() -> Result<(), TextForgeError> {
        let mut processor = TextForgeProcessor::new();
        let token = TokenWrapper::new(Box::new(Atb::new("banana")), None);

        let input = "a".repeat(100);

        let output = processor.process_single(token, &input)?;

        let expected_output = format!("{}{}", "banana", input);

        assert_eq!(output, expected_output);

        Ok(())
    }
    #[test]
    fn test_process_single_with_debug() -> Result<(), TextForgeError> {
        let mut processor: Box<dyn TextForgeProcessorMethods> = Box::new(TextForgeProcessor::new());
        let token = TokenWrapper::new(Box::new(Atb::new("banana")), None);

        let input = "a".repeat(100);

        let output = processor.process_single_with_debug(token, &input)?;

        let expected_output = format!("{}{}", "banana", input);

        assert_eq!(output, expected_output);

        Ok(())
    }

    #[test]
    fn test_read_from_file() -> Result<(), TextForgeError> {
        let mut processor = TextForgeProcessor::new();

        let identifier = processor
            .get_pipeline_from_file(Path::new("./pipelines/text/simple_pipeline.textforge"))?;

        let input_string = "Banana";
        let expected_output = "BznzbonanzanzBznznz";

        let output = processor.process_all(&identifier, input_string)?;

        println!("{} => {} == {}", input_string, output, expected_output);

        assert_eq!(
            output, expected_output,
            "Unexpected Output in read_from_file"
        );

        Ok(())
    }

    #[test]
    fn test_write_to_file() -> Result<(), TextForgeError> {
        use std::fs::File;
        use std::io::Read;

        use tempfile::Builder;

        let file = Builder::new()
            .suffix(".textforge")
            .tempfile()
            .expect("Error opening archive");

        let path = file.path();

        let mut processor = TextForgeProcessor::new();
        let identifier = processor
            .create_pipeline()
            .add_to_beginning("Banana")?
            .add_to_end("Laranja")?
            .repeat(3usize)?
            .build()
            .unwrap();

        // ✅ não ignore o erro
        processor.write_pipeline_to_file(&identifier, path)?;

        let mut opened_file = File::open(path).unwrap();
        let mut content = String::new();
        opened_file.read_to_string(&mut content).unwrap();

        // ✅ normaliza CRLF só por segurança
        let content = content.replace("\r\n", "\n");

        let expected_content = "atb Banana;\nate Laranja;\nrpt 3;\n";

        assert_eq!(
            content, expected_content,
            "Unexpected Output in test_write_to_file: content differs"
        );
        Ok(())
    }

    #[test]
    fn test_add_transform() {
        use uuid::Variant;
        let tokens: Vec<TokenWrapper> = vec![
            TokenWrapper::new(Box::new(Atb::new("Banana")), None),
            TokenWrapper::new(Box::new(Ate::new("Pizza")), None),
            TokenWrapper::new(Box::new(Rpt::new(5 as usize)), None),
        ];

        let mut processor = TextForgeProcessor::new();

        let identifier = processor.add_transform(tokens);

        let parsed = Uuid::parse_str(&identifier).expect(
            "Unexpected output in test_add_transform: Non valid UUID generated by add_transform",
        );

        assert_eq!(
            parsed.get_variant(),
            Variant::RFC4122,
            "Unexpected output in test_add_transform: Non UUID_V4"
        );
        assert_eq!(
            parsed.get_version_num(),
            4 as usize,
            "Unexpected output in test_add_transform: UUID is from different version"
        );
    }
}
