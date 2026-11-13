# Execution profiles and `.txfrc`

A profile trades memory for speed and sets safety limits. It **never**
changes the text a pipeline produces: the same pipeline, text and static
context give byte-identical output and errors under every preset
(`tests/profile_parity.rs` runs every processing method under every preset).
The only effect a limit can have is to stop a run that exceeds it, with an
error that names the limit. Pipelines cannot read the profile, and a pipeline
file never declares one: the pipeline describes the logic, the profile
describes the machine.

## `ExecutionProfile`

| Field | Meaning |
| --- | --- |
| `spool_threshold` | Bytes above which buffered processing keeps a document in a temporary file instead of memory (4 KiB to 64 GiB) |
| `limits.max_generated_bytes` | Largest text `rpt`, `padl` and `padr` may produce (default 2 GiB; `None` = unlimited) |
| `limits.max_iterations` | Largest `iter` count (default 10,000,000; `None` = unlimited) |

| Preset | `spool_threshold` |
| --- | --- |
| `balanced` (default; the behavior before profiles existed) | 1 MiB |
| `low-memory` | 64 KiB |
| `fast` | 256 MiB |

`spool_threshold` only has an effect with the `buffered` feature; `txf config
show` says so when it has none. Preset names are stable; individual fields may
change between minor versions. Fields are only added when a benchmark shows a
measurable effect; character and line index caches are not part of this
version.

```rust
use textforge::api::{ExecutionProfile, processor::TextForgeProcessor};

let mut processor = TextForgeProcessor::new().with_profile(ExecutionProfile::low_memory());
processor.set_profile(ExecutionProfile::fast());
assert_eq!(processor.profile(), &ExecutionProfile::fast());
```

The portable processor has `set_profile` and `profile`.

## Limits

Some instructions do work proportional to a parameter rather than to the
text: `rpt 99999999999;` asks for a 100 GB result, and `iter` runs its body as
many times as asked. Without a limit a typo exhausts memory, CPU, or — with
`buffered`, where a large result spools to a temporary file — the disk. The
default limits are generous and identical in every preset; past them the
instruction fails at once with a `ValidationError` that says what was needed,
the limit, and how to raise it. They apply in every processing mode, inside
blocks, `on*` and `ppjn`, and to direct `transform` calls without a context
(which use the defaults).

```rust
use textforge::api::{ExecutionProfile, Limits};

let mut profile = ExecutionProfile::fast();
profile.limits.max_generated_bytes = Some(16 << 30); // 16 GiB
let unlimited = ExecutionProfile { limits: Limits::unlimited(), ..profile };
```

## `.txfrc`

JSON read **only by the `txf` CLI**. The library and bindings never read
configuration files on their own; `ExecutionProfile::from_rc_file(path)` and
`RcFile` exist for callers that choose to. Schema: [`txfrc.schema.json`](txfrc.schema.json)
(a test keeps it in sync with the code).

```json
{
  "version": 1,
  "profile": "fast",
  "overrides": {
    "spool_threshold": "512MiB",
    "max_generated_bytes": "16GiB",
    "max_iterations": "unlimited"
  }
}
```

- `version` is required and must be 1.
- Unknown keys are errors, never ignored; so are values out of range (the
  message gives the range). A `.txfrc` cannot hold anything that changes
  results (separators, variables, regex flags, normalization modes).
- Sizes are a number of bytes or text with `B`, `KiB`, `MiB` or `GiB`. The
  limits also accept `"unlimited"`.

### Precedence in `txf`

For each value, the strongest source wins:

1. Flags: `--profile balanced|low-memory|fast`, `--spool-threshold SIZE`,
   `--max-generated-bytes SIZE|unlimited`, `--max-iterations N|unlimited`.
2. The file given with `--config PATH`.
3. `.txfrc` in the current directory, unless `--no-rc`.
4. The `balanced` preset.

Only one file is read (`--config` replaces `./.txfrc`); parent directories are
not searched and files are never merged. `txf config show` prints the
effective profile and where each value came from:

```text
$ txf config show --spool-threshold 8MiB
profile: fast (file ./.txfrc)
spool_threshold: 8388608 bytes (command-line flag)
max_generated_bytes: 2147483648 bytes (default)
max_iterations: unlimited (file ./.txfrc)
```

The watcher report is a JSON array of rows; adding the profile to it would
change its format, so the report does not include it.
