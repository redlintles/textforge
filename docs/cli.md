# The `txf` command line

`txf` runs a TextForge pipeline on a file or on standard input and prints or
saves the result. Install it with:

```sh
cargo install textforge --features cli            # add ,buffered for spool-backed runs
```

The `cli` feature turns on `bytecode` and `watchers`, so every `txf` reads and
writes bytecode and runs watchers. `txf --help` lists every option with
examples; `txf <command> --help` describes each subcommand.

## Running a pipeline

```sh
txf -f clean.textforge -i in.txt -o out.txt   # file to file
cat in.txt | txf -f clean.tft                 # standard input to standard output
txf -f clean.tft -i - -o -                    # the same, spelled out
```

| Option | Meaning |
|---|---|
| `-f, --file PIPELINE` | The pipeline to run: text (`.textforge`, `.tft`) or bytecode (`.textforgebc`, `.tfb`). |
| `-m, --mode MODE` | `auto` (default) tells bytecode from text by the first bytes of the file, as `ppjn` does; `t`/`text` and `b`/`bytecode` force a format and check its extension. |
| `-i, --input FILE` | The UTF-8 text to process. Without it, or with `-`, `txf` reads standard input (and says so on stderr when that is a terminal). The pipeline sees this path as `__INPUT_FILE__`, empty for standard input. |
| `-o, --output FILE` | Where to write the result. Without it, or with `-`, the result is printed. A regular file is replaced atomically, as `wtf` does: a failure leaves the previous file intact; the replaced file gets default permissions. Devices and FIFOs (`/dev/null`) are written directly. Missing directories are not created. |
| `-r, --read-mode MODE` | See [read modes](#read-modes). |
| `--chomp` | Set the final line break of the input aside and put it back on the result, so `dll` or `ate` act on the last visible character. |
| `-d, --debug` | Print the step-by-step trace ([debug output](debug-output.md)) instead of the result. With `--output`, the result is still written to the file. |

The pipeline is loaded and validated before the input is read, so a mistake
in it is reported at once. Relative paths inside the pipeline (`wtf`, `apf`,
`ppf`, `inf`, `ppjn`) start from the pipeline's directory, wherever `txf` runs
([file instructions](file-instructions.md)).

When the reader of the output goes away (`txf … | head`), `txf` stops and exits
with 0.

## Read modes

| Mode | Runs the pipeline | On an empty input |
|---|---|---|
| `all` (default) | once, on the whole text | once, on `""` |
| `line` | once per line, without its line break, which is put back as it was | never |
| `chunk-N` | once per piece of `N` characters | never |

Lines follow [the line rules of the engine](lines.md): LF, CRLF (whole), CR,
VT, FF, NEL, LINE SEPARATOR and PARAGRAPH SEPARATOR end a line, as for
`__LINES__` and the line instructions. `--chomp` recognizes the same
terminators; in `line` mode every line break is already set aside, so
`--chomp` adds nothing.

Chunks count characters (Unicode scalar values), never bytes, so a multibyte
character is never cut; a chunk never ends between the `\r` and the `\n` of a
CRLF (it takes the `\n`, one character more than `N`). Combining marks can
still end up in the next chunk.

Each run is independent: variables, blocks and replacement maps start empty
for every line or chunk. Errors in a piece start with `input line N ›` or
`input chunk N ›` (counted from 1, like the lines of an editor), and in a
trace each run is introduced by `=== input line N ===` or
`=== input chunk N ===`.

## Watchers

```sh
txf -f clean.tft -i in.txt --watch current_len_chars,line_count --watch-report report.json
```

`--watch METRIC` (repeatable or comma-separated; `all` for every metric) runs
the built-in watchers after every instruction and `--watch-report FILE` is
where their JSON report goes (one row per instruction). Metrics:
`current_len_bytes`, `current_len_chars`, `byte_delta`, `char_delta`,
`word_count`, `word_count_delta`, `line_count`, `is_empty`, `is_unchanged`,
`whitespace_count`, `digit_count`, `alpha_count`, `uppercase_count`,
`lowercase_count`, `unique_char_count`. Watchers need `--read-mode all` (one run,
one report) and cannot be combined with `--debug`. They add work to every
step: use them for feedback, not for production runs.

## Subcommands

| Command | Does |
|---|---|
| `txf compile PIPELINE [-o FILE]` | Writes a text pipeline as bytecode; joined pipelines (`ppjn`) are embedded, so the bytecode runs without their files. Default output: the pipeline's name with `.textforgebc`, next to it. |
| `txf decompile PIPELINE [-o FILE]` | Writes a bytecode pipeline back as text (default: `.textforge` next to it). An embedded `ppjn` becomes `ppjn "path";` again. |
| `txf check PIPELINE...` | Loads each pipeline without running it (parsing, validation, joined pipelines) and prints `NAME: ok, N instructions`; errors go to stderr and the exit status is 3 when any pipeline fails. |
| `txf config show` | Prints the effective profile and where each value came from. |

## Profiles, limits and `.txfrc`

`--profile`, `--spool-threshold`, `--max-generated-bytes` and
`--max-iterations` choose resources and safety limits; they never change the
text produced, a limit can only make a run fail. `txf` reads `.txfrc` from the
**current directory** (not from the pipeline's directory), `--config FILE`
reads another file and `--no-rc` none. Precedence, the file format and the
presets are in [execution profiles](execution-profiles.md).

## Exit status

| Status | Meaning |
|---|---|
| 0 | Success (also when the reader of the output went away) |
| 1 | The pipeline failed on the text, including errors of file instructions while running |
| 2 | Invalid arguments or configuration (`.txfrc`, profile flags, `--watch` with another read mode) |
| 3 | The pipeline could not be loaded: missing file, wrong extension, syntax, validation, a joined pipeline |
| 4 | The input could not be read or the result could not be written |

Errors are printed on stderr as a report: `error[N] Name: message`, where it
happened (`-->`), the trace of instructions from the outermost to the one that
failed (with `file:line`), the parameters of that instruction with the values
it ran with, the input, the cause and a hint. It is colored on a terminal;
`NO_COLOR` turns the color off and `CLICOLOR_FORCE` forces it. `N` is the
stable code of `TextForgeErrorCode`. See [error handling](error-handling.md).

Errors ignored by `tryi` do not change the exit status (0) and print nothing
in a normal run; `--debug` shows each one where it happened, as a warning,
and lists them at the end of the trace.

## From Rust

`utils::cli` (with the `cli` feature) exposes what `txf` uses:
`process_input` (read modes, input file, trace destination),
`load_pipeline` and `detect_pipeline_format`, `process_with_watchers`,
`write_output`, `split_final_newline` and `resolve_profile`.
