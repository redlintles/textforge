#[cfg(feature = "test_access")]
#[cfg(test)]
pub mod bytecode {
    use std::{fs::File, io::Read};

    use textforge::parser::resolve_var::TokenWrapper;

    #[cfg(test)]
    mod write_bytecode_to_file_tests {
        use std::fs;
        use std::path::PathBuf;

        use tempfile::tempdir;
        use textforge::bytecode::writer::write_bytecode_to_file;
        use textforge::parser::params::TextForgeParamTypes;
        use textforge::parser::resolve_var::TokenWrapper;
        use textforge::tokens::InstructionMethods;
        use textforge::tokens::transforms::{atb::Atb, ate::Ate, ctc::Ctc, dlf::Dlf, rpt::Rpt};

        fn parse_header(bytes: &[u8]) -> (Vec<u8>, u64, u32, &[u8]) {
            assert!(bytes.len() >= 20);
            let magic = bytes[0..8].to_vec();
            let protocol = u64::from_be_bytes(bytes[8..16].try_into().unwrap());
            let count = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let rest = &bytes[20..];
            (magic, protocol, count, rest)
        }

        /// Helper: create an empty file first so `canonicalize()` doesn't fail in `check_file_path`.
        fn touch(path: &PathBuf) {
            fs::OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(path)
                .unwrap();
        }

        /// Um exemplar de cada "forma" de instrução: 1 String, 1 Usize, 2 Usize e zero params.
        fn sample_tokens() -> Vec<TokenWrapper> {
            let mut ctc = Ctc::default();
            ctc.set_params(&[TextForgeParamTypes::Usize(0), TextForgeParamTypes::Usize(3)])
                .unwrap();

            vec![
                TokenWrapper::new(Box::new(Atb::new("Banana")), None), // String
                TokenWrapper::new(Box::new(Ate::new("Pizza")), None),  // String
                TokenWrapper::new(Box::new(Rpt::new(5_usize)), None),  // Usize
                TokenWrapper::new(Box::new(Dlf::default()), None),     // zero params
                TokenWrapper::new(Box::new(ctc), None),                // 2x Usize
            ]
        }

        /// Mesma lista de tokens, mas como instâncias soltas — usadas só pra
        /// calcular o bytecode esperado via to_bytecode() de cada uma,
        /// sem depender de literais de bytes escritos à mão.
        fn sample_instructions() -> Vec<Box<dyn InstructionMethods>> {
            let mut ctc = Ctc::default();
            ctc.set_params(&[TextForgeParamTypes::Usize(0), TextForgeParamTypes::Usize(3)])
                .unwrap();

            vec![
                Box::new(Atb::new("Banana")),
                Box::new(Ate::new("Pizza")),
                Box::new(Rpt::new(5_usize)),
                Box::new(Dlf::default()),
                Box::new(ctc),
            ]
        }

        #[test]
        fn writes_header_and_all_token_bytecodes_in_order() {
            let dir = tempdir().unwrap();
            let path: PathBuf = dir.path().join("out.textforgebc");

            touch(&path);

            let tokens = sample_tokens();

            write_bytecode_to_file(&path, tokens).unwrap();

            let bytes = fs::read(&path).unwrap();
            let (magic, protocol, count, rest) = parse_header(&bytes);

            let expected_magic = vec![38, 235, 245, 8, 244, 137, 1, 179];
            assert_eq!(magic, expected_magic);
            assert_eq!(protocol, 1);
            assert_eq!(count, 5);

            let mut expected_payload: Vec<u8> = Vec::new();
            for instruction in sample_instructions() {
                expected_payload.extend_from_slice(&instruction.to_bytecode().unwrap());
            }

            assert_eq!(rest, expected_payload.as_slice());
        }

        #[test]
        fn instruction_count_is_zero_when_no_tokens() {
            let dir = tempdir().unwrap();
            let path: PathBuf = dir.path().join("empty.textforgebc");

            touch(&path);

            let tokens: Vec<TokenWrapper> = vec![];
            write_bytecode_to_file(&path, tokens).unwrap();

            let bytes = fs::read(&path).unwrap();
            let (_magic, _protocol, count, rest) = parse_header(&bytes);

            assert_eq!(count, 0);
            assert!(rest.is_empty(), "no tokens => no payload bytes");
        }

        #[test]
        fn invalid_extension_is_rejected_by_check_file_path() {
            let dir = tempdir().unwrap();
            let path: PathBuf = dir.path().join("wrong.txt");

            touch(&path);

            let tokens = sample_tokens();

            let err = write_bytecode_to_file(&path, tokens).unwrap_err();

            let msg = format!("{err:?}");
            assert!(
                msg.contains("ValidationError")
                    || msg.contains("check_file_path")
                    || msg.contains("textforgebc"),
                "expected an extension/path validation error, got: {msg}"
            );
        }

        #[test]
        fn directory_path_is_rejected_by_check_file_path() {
            let dir = tempdir().unwrap();
            let path_is_dir: PathBuf = dir.path().join("some_dir.textforgebc");

            fs::create_dir_all(&path_is_dir).unwrap();

            let tokens = sample_tokens();

            let err = write_bytecode_to_file(&path_is_dir, tokens).unwrap_err();

            let msg = format!("{err:?}");
            assert!(
                msg.contains("Path is a directory") || msg.contains("ValidationError"),
                "expected directory validation error, got: {msg}"
            );
        }
    }

    #[test]
    fn test_write_bytecode_to_file() {
        use tempfile::Builder;
        use textforge::bytecode::writer::write_bytecode_to_file;
        use textforge::tokens::transforms::{atb::Atb, ate::Ate, rpt::Rpt};
        let file = Builder::new()
            .suffix(".textforgebc")
            .prefix("output_")
            .tempfile()
            .unwrap();

        let path = file.path();

        let tokens: Vec<TokenWrapper> = vec![
            TokenWrapper::new(Box::new(Atb::new("Banana")), None),
            TokenWrapper::new(Box::new(Ate::new("Pizza")), None),
            TokenWrapper::new(Box::new(Rpt::new(5_usize)), None),
        ];

        let mut header: Vec<u8> = Vec::new();

        let magic_number: Vec<u8> = vec![38, 235, 245, 8, 244, 137, 1, 179];
        let protocol_version = (1_u64).to_be_bytes();
        let instruction_count = (tokens.len() as u32).to_be_bytes();

        header.extend_from_slice(&magic_number);
        header.extend_from_slice(&protocol_version);
        header.extend_from_slice(&instruction_count);

        let mut expected_content: Vec<u8> = vec![];
        expected_content.extend_from_slice(&header);

        for t in tokens.iter() {
            expected_content.extend_from_slice(&t.to_bytecode().unwrap());
        }
        let _ = write_bytecode_to_file(path, tokens);

        let mut opened_file = File::open(path).unwrap();

        let mut content: Vec<u8> = Vec::new();
        opened_file.read_to_end(&mut content).unwrap();

        assert_eq!(
            content, expected_content,
            "Unexpected Output in test_write_to_file: content differs"
        );
    }

    #[test]
    fn test_read_bytecode_from_file() {
        use std::path::Path;
        use tempfile::Builder;

        use textforge::{
            api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
            bytecode::{reader::read_bytecode_from_file, writer::write_bytecode_to_file},
            tokens::transforms::{atb::Atb, ate::Ate, rpt::Rpt},
        };

        // atb "Banana" + ate "Pizza" + rpt 5 sobre "Coxinha":
        // prepend -> append -> repete o resultado 5 vezes.
        let tokens: Vec<TokenWrapper> = vec![
            TokenWrapper::new(Box::new(Atb::new("Banana")), None),
            TokenWrapper::new(Box::new(Ate::new("Pizza")), None),
            TokenWrapper::new(Box::new(Rpt::new(5_usize)), None),
        ];

        let tmp = Builder::new()
            .prefix("banana_")
            .suffix(".textforgebc")
            .tempfile()
            .unwrap();
        let file_path = tmp.path().to_path_buf();
        write_bytecode_to_file(Path::new(&file_path), tokens).unwrap();

        use std::fs;
        let data = fs::read(&file_path).unwrap();
        eprintln!("len = {}", data.len());
        eprintln!("header bytes = {:02x?}", &data[..(20).min(data.len())]);
        eprintln!(
            "first body bytes = {:02x?}",
            &data[20..(20 + 16).min(data.len())]
        );

        assert!(
            file_path.exists(),
            "writer não criou o arquivo: {:?}",
            file_path
        );

        let read_tokens = read_bytecode_from_file(Path::new(&file_path)).unwrap();

        let input = "Coxinha";
        // "Banana" + "Coxinha" + "Pizza", repetido 5x — bate com atb "Banana",
        // ate "Pizza" e rpt 5 aplicados nessa ordem.
        let expected_output = "BananaCoxinhaPizza".repeat(5);

        let mut processor: Box<dyn TextForgeProcessorMethods> = Box::new(TextForgeProcessor::new());
        println!("read_tokens len {}", read_tokens.len());
        let identifier = processor.add_transform(read_tokens);

        let output = processor
            .process_all_with_debug(&identifier, input)
            .unwrap();

        assert_eq!(output, expected_output);
    }
}
