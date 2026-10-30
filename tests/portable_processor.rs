//! Full processor parity through production public portable types.
use std::path::PathBuf;
use textforge::api::portable::{
    Argument, BatchPool, ErrorStage, FilePath, FileTask, InstructionSpec, PortableProcessor,
};

fn op(name: &str, arguments: Vec<Argument>) -> InstructionSpec {
    InstructionSpec::new(name.into(), arguments)
}

/// Every test owns its temporary directory; cleanup never touches project data.
struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "textforge-portable-parity-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> FilePath {
        FilePath::from_text(self.0.join(name).to_str().unwrap().into())
    }
    fn write(&self, name: &str, text: &str) -> FilePath {
        std::fs::write(self.0.join(name), text).unwrap();
        self.path(name)
    }
    fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.0.join(name)).unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn processor() -> (PortableProcessor, String) {
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![
            op("atb", vec![Argument::Text("🍌".into())]),
            op("ate", vec![Argument::Text("é".into())]),
        ])
        .unwrap();
    (processor, id)
}

#[test]
fn registration_inspection_cloning_and_removal_have_owned_snapshots() {
    let (mut processor, id) = processor();
    let snapshot = processor.get_transform_vec(&id).unwrap();
    assert_eq!(
        snapshot,
        vec![
            op("atb", vec![Argument::Text("🍌".into())]),
            op("ate", vec![Argument::Text("é".into())])
        ]
    );
    assert_eq!(processor.get_text_transform_vec(&id).unwrap().len(), 2);
    let clone = processor.clone_pipeline(&id).unwrap();
    let imported = processor.add_transform(snapshot).unwrap();
    assert_eq!(processor.list_transforms().len(), 3);
    processor.show_transforms();
    processor.remove_transform(&id).unwrap();
    assert!(!processor.transform_exists(&id));
    assert_eq!(processor.run_transform(&clone, "x").unwrap(), "🍌xé");
    assert_eq!(processor.process_all(&imported, "x").unwrap(), "🍌xé");
    assert_eq!(processor.get_transform_vec(&id).unwrap_err().code, 3);
}

#[test]
fn invalid_registration_is_atomic_and_nested_snapshots_preserve_references() {
    let mut processor = PortableProcessor::new();
    let error = processor
        .add_transform(vec![op("atb", vec![Argument::Number(1)])])
        .unwrap_err();
    assert_eq!(error.stage, ErrorStage::Validation);
    assert!(processor.list_transforms().is_empty());
    let original = op(
        "iter",
        vec![
            Argument::Number(2),
            Argument::Instruction(Box::new(op(
                "atb",
                vec![Argument::Variable("prefix".into())],
            ))),
        ],
    );
    let id = processor
        .add_transform(vec![
            op(
                "val",
                vec![Argument::Text("prefix".into()), Argument::Text("🍌".into())],
            ),
            original.clone(),
        ])
        .unwrap();
    assert_eq!(processor.get_transform_vec(&id).unwrap()[1], original);
    assert_eq!(processor.process_all(&id, "x").unwrap(), "🍌🍌x");
}

#[test]
fn single_and_debug_methods_delegate_to_core_execution() {
    let (mut processor, id) = processor();
    let single = op("atb", vec![Argument::Text("🍌".into())]);
    assert_eq!(
        processor.process_single(single.clone(), "x").unwrap(),
        "🍌x"
    );
    assert_eq!(
        processor.process_single_with_debug(single, "x").unwrap(),
        "🍌x"
    );
    assert_eq!(processor.process_all_with_debug(&id, "x").unwrap(), "🍌xé");
    assert_eq!(
        processor
            .process_all_with_debug("missing", "x")
            .unwrap_err()
            .code,
        3
    );
}

#[test]
fn text_persistence_and_multiple_pipeline_file_batches_preserve_order() {
    let files = Files::new();
    let (mut processor, id) = processor();
    let pipeline_path = files.write("pipeline.textforge", "");
    processor
        .write_pipeline_to_file(&id, &pipeline_path)
        .unwrap();
    let loaded = processor.get_pipeline_from_file(&pipeline_path).unwrap();
    assert_eq!(processor.process_all(&loaded, "x").unwrap(), "🍌xé");
    let input = files.write("in.txt", "x");
    processor
        .process_file(&id, &input, &files.path("single.txt"))
        .unwrap();
    assert_eq!(files.read("single.txt"), "🍌xé");
    let tasks = vec![
        FileTask {
            input: input.clone(),
            pipeline_id: id.clone(),
            output: files.path("first.txt"),
        },
        FileTask {
            input: files.path("missing.txt"),
            pipeline_id: id.clone(),
            output: files.path("failed.txt"),
        },
        FileTask {
            input,
            pipeline_id: loaded,
            output: files.path("third.txt"),
        },
    ];
    for result in [
        processor.process_batch(tasks.clone()),
        processor.process_batch_in_pool(&BatchPool::new(2).unwrap(), tasks),
    ] {
        assert!(result[0].is_ok());
        assert!(result[1].is_err());
        assert!(result[2].is_ok());
    }
    assert_eq!(files.read("first.txt"), files.read("third.txt"));
    assert!(!files.0.join("failed.txt").exists());
}

#[test]
fn native_paths_are_roundtripped_without_loss() {
    let path = FilePath::from_text("é.txt".into());
    assert_eq!(path.to_text().as_deref(), Some("é.txt"));
    #[cfg(unix)]
    {
        let path = FilePath::from_native_bytes(vec![b'x', 0xff]).unwrap();
        assert!(path.to_text().is_none());
        assert_eq!(path.to_native_bytes().unwrap(), vec![b'x', 0xff]);
        assert!(FilePath::from_windows_wide(vec![65]).is_err());
        let files = Files::new();
        use std::os::unix::ffi::OsStrExt;
        let mut bytes = files.0.as_os_str().as_bytes().to_vec();
        bytes.extend_from_slice(b"/nonutf8-\xff.txt");
        let input = FilePath::from_native_bytes(bytes.clone()).unwrap();
        use std::os::unix::ffi::OsStringExt;
        std::fs::write(PathBuf::from(std::ffi::OsString::from_vec(bytes)), "x").unwrap();
        let (mut processor, id) = processor();
        processor
            .process_file(&id, &input, &files.path("out.txt"))
            .unwrap();
        assert_eq!(files.read("out.txt"), "🍌xé");
    }
    #[cfg(windows)]
    {
        let units = vec![65, 0xd800];
        let path = FilePath::from_windows_wide(units.clone()).unwrap();
        assert_eq!(path.to_windows_wide().unwrap(), units);
        assert!(FilePath::from_native_bytes(vec![65]).is_err());
    }
}

#[cfg(feature = "bytecode")]
#[test]
fn bytecode_persistence_preserves_numeric_references_and_snapshots() {
    let files = Files::new();
    let mut processor = PortableProcessor::new();
    let id = processor
        .add_transform(vec![
            op(
                "val",
                vec![Argument::Text("n".into()), Argument::Text("2".into())],
            ),
            op("rpt", vec![Argument::Variable("n".into())]),
        ])
        .unwrap();
    let path = files.write("pipeline.textforgebc", "");
    processor.write_to_bytecode_file(&id, &path).unwrap();
    let loaded = processor.read_from_bytecode_file(&path).unwrap();
    assert_eq!(
        processor.get_transform_vec(&id).unwrap(),
        processor.get_transform_vec(&loaded).unwrap()
    );
    assert_eq!(processor.process_all(&loaded, "é").unwrap(), "éé");
}

#[cfg(feature = "watchers")]
#[test]
fn custom_dynamic_watchers_preserve_context_replacement_schedule_and_reset() {
    use std::{cell::RefCell, rc::Rc};
    use textforge::api::portable::{Observation, Observers};
    let files = Files::new();
    let (mut processor, id) = processor();
    let mut observers = Observers::new();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let capture = seen.clone();
    let name = format!("dynamic-🍌-{}", 42);
    observers.set(
        name.clone(),
        Box::new(move |context: Observation| {
            capture.borrow_mut().push(context.clone());
            Ok(context.current.chars().count().to_string())
        }),
    );
    observers.schedule_all(&name).unwrap();
    observers.set_builtin("words".into(), "word_count").unwrap();
    observers.schedule_all("words").unwrap();
    assert_eq!(
        processor
            .process_all_with_watchers(&id, "x", &mut observers, &files.path("report.json"))
            .unwrap(),
        "🍌xé"
    );
    let contexts = seen.borrow();
    assert_eq!(contexts.len(), 2);
    assert_eq!(contexts[0].before, "x");
    assert_eq!(contexts[0].current, "🍌x");
    assert_eq!(contexts[0].after.as_deref(), Some("🍌xé"));
    assert_eq!(contexts[1].after, None);
    drop(contexts);
    let rows: serde_json::Value = serde_json::from_str(&files.read("report.json")).unwrap();
    assert_eq!(rows[0][&name], "2");
    assert_eq!(rows[1][&name], "3");
    observers.write_report(&files.path("reset.json")).unwrap();
    assert_eq!(files.read("reset.json"), "[]");
    observers.set(
        name.clone(),
        Box::new(|_: Observation| Ok("replacement".into())),
    );
    processor
        .process_all_with_watchers(&id, "x", &mut observers, &files.path("report.json"))
        .unwrap();
    let rows: serde_json::Value = serde_json::from_str(&files.read("report.json")).unwrap();
    assert_eq!(rows[0][&name], "replacement");
    observers.reset();
}

#[cfg(feature = "watchers")]
#[test]
fn observer_failures_abort_processing_without_publishing_a_partial_report() {
    use textforge::api::portable::{Observation, Observers};
    let files = Files::new();
    let report = files.write("report.json", "keep");
    let (mut processor, id) = processor();
    let mut observers = Observers::new();
    observers.set(
        "failure".into(),
        Box::new(|_: Observation| Err("foreign callback failed".into())),
    );
    observers.schedule_all("failure").unwrap();
    let error = processor
        .process_all_with_watchers(&id, "x", &mut observers, &report)
        .unwrap_err();
    assert_eq!(error.stage, ErrorStage::Callback);
    assert_eq!(error.message, "foreign callback failed");
    assert_eq!(files.read("report.json"), "keep");
    assert!(observers.schedule_all("missing").is_err());
    assert!(observers.set_builtin("x".into(), "missing").is_err());
    assert_eq!(Observers::builtin_names().len(), 15);
}

#[cfg(feature = "buffered")]
fn options(files: &Files) -> textforge::api::portable::BufferOptions {
    textforge::api::portable::BufferOptions {
        memory_threshold: 1,
        spill_directory: FilePath::from_text(files.0.to_str().unwrap().into()),
    }
}

#[cfg(feature = "buffered")]
#[test]
fn buffered_processing_file_batches_and_byte_ranges_preserve_spooling() {
    use textforge::api::portable::{BufferTask, PortableBuffer};
    let files = Files::new();
    let (mut processor, id) = processor();
    let input = PortableBuffer::from_text("x", options(&files)).unwrap();
    let mut result = processor.process_all_buffered(&id, input).unwrap();
    assert!(result.is_spilled());
    assert_eq!(result.read_at(0, u64::MAX).unwrap(), "🍌xé".as_bytes());
    assert!(result.read_at(u64::MAX, 10).unwrap().is_empty());
    result.spill().unwrap();
    assert_eq!(result.write_to_file(&files.path("buffer.txt")).unwrap(), 7);
    assert_eq!(result.into_string().unwrap(), "🍌xé");
    let input = files.write("in.txt", "x");
    assert_eq!(
        processor
            .process_file_buffered(&id, &input, options(&files))
            .unwrap()
            .into_string()
            .unwrap(),
        "🍌xé"
    );
    let input_buffer = PortableBuffer::from_file(&input, options(&files)).unwrap();
    assert!(!input_buffer.is_empty());
    assert_eq!(
        processor
            .process_single_buffered("atb 🍌;", input_buffer)
            .unwrap()
            .into_string()
            .unwrap(),
        "🍌x"
    );
    let results = processor.process_batch_buffered(
        vec![
            BufferTask {
                pipeline_id: id.clone(),
                input,
            },
            BufferTask {
                pipeline_id: id,
                input: files.path("missing.txt"),
            },
        ],
        options(&files),
    );
    let mut results = results.into_iter();
    assert!(results.next().unwrap().unwrap().is_spilled());
    assert!(results.next().unwrap().is_err());
}

#[cfg(feature = "buffered")]
#[test]
fn stream_hooks_validate_counts_preserve_utf8_and_retry_partial_writes() {
    use std::{cell::RefCell, rc::Rc};
    use textforge::api::portable::PortableBuffer;
    let files = Files::new();
    let mut bytes = "🍌é"
        .as_bytes()
        .iter()
        .copied()
        .collect::<std::collections::VecDeque<_>>();
    let mut buffer = PortableBuffer::from_reader(
        Box::new(move |_maximum| Ok(bytes.pop_front().into_iter().collect())),
        options(&files),
    )
    .unwrap();
    let output = Rc::new(RefCell::new(Vec::new()));
    let capture = output.clone();
    assert_eq!(
        buffer
            .copy_to(Box::new(move |bytes: Vec<u8>| {
                if let Some(byte) = bytes.first() {
                    capture.borrow_mut().push(*byte);
                    Ok(1)
                } else {
                    Ok(0)
                }
            }))
            .unwrap(),
        6
    );
    assert_eq!(*output.borrow(), "🍌é".as_bytes());
    assert!(
        buffer
            .copy_to(Box::new(|bytes: Vec<u8>| Ok(bytes.len() as u64 + 1)))
            .is_err()
    );
    assert!(
        buffer
            .copy_to(Box::new(|_: Vec<u8>| Err("write failed".into())))
            .is_err()
    );
    assert!(
        PortableBuffer::from_reader(
            Box::new(|maximum| Ok(vec![0; maximum as usize + 1])),
            options(&files)
        )
        .is_err()
    );
    let mut once = true;
    assert!(
        PortableBuffer::from_reader(
            Box::new(move |_| {
                if once {
                    once = false;
                    Ok(vec![0xff])
                } else {
                    Ok(vec![])
                }
            }),
            options(&files)
        )
        .is_err()
    );
}

#[cfg(feature = "watchers")]
#[test]
fn manually_running_observers_uses_the_same_report_and_reset_contract() {
    use textforge::api::portable::{Observation, Observers};
    let files = Files::new();
    let mut observers = Observers::new();
    observers
        .set_builtin("bytes".into(), "current_len_bytes")
        .unwrap();
    observers.schedule_all("bytes").unwrap();
    observers
        .run(Observation {
            current: "🍌".into(),
            before: "".into(),
            after: None,
            instruction: "manual".into(),
        })
        .unwrap();
    observers.write_report(&files.path("manual.json")).unwrap();
    let report: serde_json::Value = serde_json::from_str(&files.read("manual.json")).unwrap();
    assert_eq!(report[0]["bytes"], "4");
    observers.reset();
    observers.write_report(&files.path("manual.json")).unwrap();
    assert_eq!(files.read("manual.json"), "[]");
}
