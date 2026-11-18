# Error handling

How TextForge reports errors, and how a pipeline goes on after an
instruction that fails (`tryi`). Since 0.4.0.

## What an error carries

Every error is a `TextForgeError` (`src/utils/errors.rs`):

| Field or method | What it holds |
|---|---|
| `error_code` | The kind of error and its message. `get_error_code()` is its stable number (1–31), `name()` its stable name (`IndexOutOfRange`), `message()` the message, `hint()` a general suggestion when there is one. |
| `instruction` | Where it happened: the instruction's source line (`dlc 9 9;`), with `file:line ›`, `ppjn file:line ›` or `input line N ›` in front when the context adds them. |
| `input` | What it is about: an excerpt of the text (never the whole document, rule 20), a parameter or a path. |
| `trace()` | The instructions the error left, **outermost first**; the last one is the instruction that failed. Each frame has the instruction, its `file:line` when the pipeline came from a file, and a note from the container (`__COUNTER__ = 1 (iteration 2 of 3)`, ``block `b`, step 2 of 5``). |
| `parameters()` | The parameters of the instruction that failed: name (from the syntax, `start`), the value it ran with, and how it was written when that differs (`"[[n + 2]]"`, `{{n}}`). |
| `cause()` | The error behind this one: the operating system's I/O error, the regex error, the invalid UTF-8 position. |

The trace, parameters and cause live in `details`, which is `None` for errors
that did not come from a running instruction (a parse error, a missing
pipeline). They are added **only while an error leaves the pipeline**
(`TokenWrapper::locate` and the containers' `with_note`): a run that succeeds
builds none of it. Two errors compare equal when their code, instruction and
input are equal; details are diagnostics and are not compared.

## Display and report

As in Python (`str(e)` against the traceback) and Java (`getMessage()` against
the stack trace), there are two presentations:

- `Display` is **one plain line**, without color, for logs and messages:
  `error[12] IndexOutOfRange: the range starts at character 9, but the text has 1 character (in: dlc 9 9;)`.
  Before 0.4.0 it was three colored lines (`error[12]: …`, `in:`, `input:`).
- `report()` is the full report, colored on a terminal (`NO_COLOR` and
  `CLICOLOR` are honored; `.color(false)` forces plain text). `txf` prints it
  on standard error when a run fails:

```text
error[12] IndexOutOfRange: the range starts at character 2, but the text has 1 character (valid indices 0 to 0)
  --> ppjn q.textforge:1 › dlc '[[__COUNTER__ + 1]]' 4;

  Trace (outermost first):
      r.textforge:1  ppjn q.textforge;
      q.textforge:1  iter 3 times dlc '[[__COUNTER__ + 1]]' 4;  · __COUNTER__ = 1 (iteration 2 of 3)
    → q.textforge:1  dlc 2 4;  ← failed here

  Parameters:
    start = 2   (written "[[__COUNTER__ + 1]]")
    end   = 4

  Input: a

  Hint: indices count characters, words and lines from 0; the last character is __LEN_CHARS__ - 1 and the last line __LINES__ - 1 (see docs/indices-and-ranges.md)
```

The failing instruction is shown with the values it ran with (`dlc 2 4;`);
the containers around it as they were written. The layout of the report may
change between versions: programs should read the fields, not parse it.

The portable `BindingError` carries the same details (`kind`, `trace`,
`parameters`, `cause`, `hint`, `ignorable`) and the uncolored `report`; see
[portable parity](portable-parity.md).

## Error codes

The numbers are stable. 31 is new in 0.4.0: the safety limits used to be
`ValidationError` (20), like a structural rule, so a caller could not tell
them apart. Codes 7 and 11 are never created and stay reserved. The
[inventory](audits/0.4.0-error-inventory.md) lists where each code is created.

| Code | Name | Ignorable by `tryi` |
|---|---|---|
| 1–30 | `FileNotFound` … `InvalidExprError` | yes |
| 31 | `LimitExceededError` (`max_generated_bytes`, `max_iterations`) | **no** |

`TextForgeErrorCode::ignorable()` is a match without a wildcard, so a new code
has to decide.

## `tryi`: skip an instruction that fails

```text
tryi {instruction};
```

When the instruction succeeds, `tryi` is the same as the instruction. When it
fails, the pipeline continues **as if it had never run**: the next instruction
receives the text `tryi` received, and the variables, blocks, replacement maps
and `end` are as they were before it. The builder method is
`try_ignore(instruction)`; the portable mnemonic is `tryi`.

```text
var total = 0;
blk calc assoc mutv total = 10;
blk calc assoc dlc 9 9;     // fails on a short text
tryi cblk calc;
ate {{total}};              // "a" → "a0": total is back to 0
```

### Why a snapshot

The text needs nothing: an instruction receives it and returns a new one, so
`tryi` returns what it received. An instruction that runs others (`cblk`,
`iter`, `ifdc`, `onln`, `onck`, `onlni`, another `tryi`) can change variables
before it fails: in the example above, `mutv` ran before `dlc` failed. Without
a snapshot the text would say "the block did not run" and `total` would say
"it ran halfway", and in a loop the value would depend on the iteration that
failed. So `tryi` opens a snapshot before such an instruction and puts the
context back on failure. Other instructions fail before they change anything
(their parameters are resolved first), so they take no snapshot.

A snapshot copies nothing when it opens: while it is open, the context records
the value of each variable, block and replacement map before its first change
(an undo journal, `GlobalExecutionContext::snapshot`), and a failure puts
those values back, newest first. Without an open snapshot a change costs one
check. Copying the variables instead cost 65 µs per `tryi cblk` with 1,000
variables; the journal costs the same with 0 or 1,000.

A failed `cblk` used to lose its block (the block was taken out to run it).
Inside `tryi` the block is put back, so it can run again.

### What does not go back

| | Back as before? | Why |
|---|---|---|
| Text | yes | `tryi` returns the text it received, without copying it |
| Variables, blocks, replacement maps, `end` | yes | snapshot |
| Safety limits | — | **never ignored**: a `LimitExceededError` stops the pipeline as without `tryi`. The user sets the limits for the whole run (`.txfrc`, `txf` flags, `ExecutionProfile`), not the pipeline (rule 19). |
| Watcher events already emitted | no | they record what happened |
| A file `wtf` already wrote | no | an effect outside the pipeline. `wtf` replaces its file atomically, so writing it again (a retry, or a later `wtf` to the same path) works and leaves the last text. `apf`, `ppf` and `inf` only read files, so they go back with the text. |

### Where to see ignored errors

`process_all` still returns only the text: `tryi` is silent in normal runs and
`txf` without `--debug` prints nothing about it. The **debug methods**
(`process_all_with_debug`, `process_single_with_debug`, the buffered engine,
`txf --debug`) show each ignored error where it happened, as a warning with
its trace and parameters, followed by the variables that were put back:

```text
warning[12] IndexOutOfRange (ignored by tryi): character index 9 is out of range: the text has 2 characters (valid indices 0 to 1)
  --> dls 9;

  Trace (outermost first):
      p.textforge:4  cblk calc;  · block `calc`, step 2 of 2
    → p.textforge:3  dls 9;  ← failed here
  …
  The instruction was skipped: the text and the variables are as they were before it, and the pipeline continued.
  state restored: variables total
```

and list them at the end of the trace (at most 50, then a count):

```text
1 error ignored by tryi:
  warning[12] IndexOutOfRange at p.textforge:3: dls 9; — character index 9 is out of range: …
```

Errors ignored inside a joined pipeline (`ppjn`) appear in the same trace.

### Rules

- `tryi` follows the structural rules of `iter`: `rmapd` cannot be inside it,
  and at the top level it holds one instruction (`tryi iter … ` needs a block,
  like `iter iter …`).
- Only errors raised **while the instruction runs** reach `tryi`; a pipeline
  that does not parse or validate fails when it is loaded.
- Cost: nothing measurable for pipelines without `tryi`; for an instruction
  that runs others, about 0.1 µs to open and close the snapshot, whatever the
  number of variables (see [the benchmark](benchmarks/tryi/README.md)).

## The `ErrorManager`

Up to 0.3.2 the processor copied every error it returned into a private
`ErrorManager` that nothing read and that grew for the processor's lifetime.
It no longer does; `ErrorManager` and `utils::apply::apply_transform` are
deprecated and will be removed in a later version.
