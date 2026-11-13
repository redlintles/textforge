# Execution profiles and `.txfrc`

A profile trades memory for speed. It **never** changes results: the same
pipeline, text and static context give byte-identical output and errors under
every profile (`tests/profile_parity.rs` runs every processing method under
every preset). Pipelines cannot read the profile, and a pipeline file never
declares one: the pipeline describes the logic, the profile describes the
machine.

## `ExecutionProfile`

| Field | Meaning |
| --- | --- |
| `spool_threshold` | Bytes above which buffered processing keeps a document in a temporary file instead of memory (4 KiB to 64 GiB) |

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
    "spool_threshold": "512MiB"
  }
}
```

- `version` is required and must be 1.
- Unknown keys are errors, never ignored; so are values out of range (the
  message gives the range). A `.txfrc` cannot hold anything that changes
  results (separators, variables, regex flags, normalization modes).
- Sizes are a number of bytes or text with `B`, `KiB`, `MiB` or `GiB`.

### Precedence in `txf`

For each value, the strongest source wins:

1. Flags: `--profile balanced|low-memory|fast`, `--spool-threshold SIZE`.
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
```

The watcher report is a JSON array of rows; adding the profile to it would
change its format, so the report does not include it.
