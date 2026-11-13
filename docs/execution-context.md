# Execution context and built-in variables

Each run of a pipeline gets a fresh `GlobalExecutionContext` (variables, blocks,
replacement maps) and a read-only **static context** with facts about the run.
Pipelines read those facts, and two values derived from the current text,
through **built-in variables**.

## Built-in variables

| Name | Type | Value |
| --- | --- | --- |
| `__PIPELINE_NAME__` | text | Name of the running pipeline (see below) |
| `__TEXTFORGE_VERSION__` | text | Version of the TextForge core (`CARGO_PKG_VERSION`) |
| `__INPUT_FILE__` | text | Input file path; **empty** when the input is not a file |
| `__LEN_CHARS__` | integer | Characters (Unicode scalar values) in the current text |
| `__LINES__` | integer | Logical lines in the current text, as defined in [lines](lines.md) |

They are read like any variable: `{{__LINES__}}` in a parameter,
`[[__LINES__ - 1]]` in an expression, or as a numeric parameter:

```text
onln '[[__LINES__]]' apply ate '.';     // the last line, whatever the length
ate ' ({{__LINES__}} lines, {{__LEN_CHARS__}} chars)';
atb '{{__PIPELINE_NAME__}}: ';
```

`__COUNTER__`, set by `iter`, is not a built-in in this sense: it is a normal
variable that `iter` defines (see `iter`).

### Text-derived values

`__LEN_CHARS__` and `__LINES__` always describe **the text the running
instruction receives**:

- After every instruction they reflect the new text; nothing is stored.
- Inside `onln`, `onck` and `onlni` they describe the selected section, like
  every index used there; inside `iter` they describe the text at the start of
  each iteration's step.
- A non-UTF-8 input path is converted lossily for `__INPUT_FILE__`.

They are computed only when a parameter reads them, at most once per
instruction's parameter list. A pipeline that never reads them pays nothing.
Counting `__LINES__` costs about 30 ms per 100 MB, `__LEN_CHARS__` about
5 ms ([benchmark](benchmarks/builtin-variables/README.md)). With the
`buffered` feature, a document spooled to disk is counted in 64 KiB chunks
without loading it into memory.

Where no text is being processed (for example `to_text_line_resolved`), the
text-derived values fail with `RequiredContextError`.

### Reserved names

`var`, `val`, `mutv` and `eval … in` cannot define a built-in name: it is a
`ValidationError` when the pipeline is parsed (text or bytecode), built, or,
for a name computed at execution time, when the instruction runs. Built-ins
cannot be changed. Other names starting with `__` (including `__COUNTER__`)
remain definable, so existing pipelines keep working; treat the `__` prefix as
reserved for future built-ins.

## Pipeline names

A pipeline's name is its identifier, or the file name without extension when
it was read with `get_pipeline_from_file` or `read_from_bytecode_file`. Set it
with `TextForgeProcessor::set_pipeline_name` (portable:
`PortableProcessor::set_pipeline_name`); read it with `pipeline_name`.

```rust
use textforge::api::TextForgeBuilderMethods;
use textforge::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};

let mut processor = TextForgeProcessor::new();
let id = processor.create_pipeline().add_to_end(" by {{__PIPELINE_NAME__}}")?.build()?;
processor.set_pipeline_name(&id, "cleanup")?;
assert_eq!(processor.process_all(&id, "done")?, "done by cleanup");
# Ok::<(), textforge::utils::errors::TextForgeError>(())
```

## Static context

The facts come from `StaticContext` (`src/context/static_context.rs`). It is
built by the processor, never by users, and does not change during a run.
Every run without an input file shares the pipeline's static context through
an `Arc`; `process_file` and `process_batch` give each run one carrying its
input path. It also holds `GlobalFunctions`, the registry of functions
available to `[[ ]]` expressions. It is empty in this version and has no public
registration API yet.

All variable reads go through one lookup (`context::builtins::VarReader`):
built-ins first, then stored variables.
