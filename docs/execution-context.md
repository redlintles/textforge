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

### Names

A variable name is a letter or `_` followed by letters, digits or `_`
(ASCII). `var`, `val`, `mutv` and `eval … in` reject any other name with a
`ValidationError` (`var a-b = 1;`, `var 1a = 1;`); up to 0.3.1 they accepted
it, but `{{a-b}}` could never read it.

Defining a name again with `var` or `val` replaces it, unless it was defined
with `val`: a `val` cannot be defined again, changed with `mutv` or written by
`eval … in` (`NonMutableVariableError`). Variables have no scopes, so a `val`
inside `iter`, or in a block run more than once, fails from the second time;
use `var` there. Up to 0.3.1 a second `val` or `var` replaced a `val`.

### Expressions

`[[ ]]` and `eval` use [evalexpr](https://docs.rs/evalexpr). TextForge values
are integers and text: floats and booleans are not supported. A variable whose
text is an integer is a number in an expression; any other value, including
`2.5` and `true`, is text, so `var a = 2.5;` followed by `[[a + 1]]` is an
error. The result of a whole expression must be an integer or text:
`[[7 / 2.0]]` and `[[1 < 2]]` are errors. Comparisons and decimal numbers may
still appear inside it, as in `[[if(a > 1, "yes", "no")]]`. Up to 0.3.1 a
decimal or boolean result was written as text (`3.5`, `true`). `len(text)` counts characters, like
`__LEN_CHARS__` and every index (up to 0.3.1 it counted bytes).

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
