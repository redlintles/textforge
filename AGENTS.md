# AGENTS.md

Guidance for AI agents and contributors working on **TextForge**.

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Terminology](#2-terminology)
3. [How It Works](#3-how-it-works)
4. [Repository Layout](#4-repository-layout)
5. [Usage Modes](#5-usage-modes)
6. [Textual DSL](#6-textual-dsl)
7. [Interpolation](#7-interpolation)
8. [Bytecode Format](#8-bytecode-format)
9. [Watchers](#9-watchers)
10. [Core Types](#10-core-types)
11. [Cargo Features](#11-cargo-features)
12. [Common Tasks](#12-common-tasks)
13. [Golden Rules](#13-golden-rules)
14. [Pre-Commit Checklist](#14-pre-commit-checklist)
15. [Human Approval Required](#15-human-approval-required)
16. [Dependencies](#16-dependencies)
17. [bindings](#17-bindings)

---

## 1. Project Overview

TextForge is a **DSL for writing text-transformation pipelines**. A pipeline is a sequence of instructions, split into two categories:

| Category | Purpose |
|---|---|
| **Transforms** | Modify the input text directly. |
| **Instructions** | Build logical constructs: `if`, variables, `blk` blocks (reusable instruction groups), and more. |

Key design facts:

- **Linear DSL.** No recursion, no AST. Each instruction is a single line. High-level implementations (e.g. the Rust builder) can give the *illusion* of recursion/nesting.
- **Trait-object architecture.** Instructions are implemented as trait objects to keep maintenance simple and to avoid giant `match` statements and enums.
- **Published on crates.io**, current version: `0.1.7`.
- **Legacy name.** The project was formerly called **ATP** ("Advanced Text Processor"). Some doctests and internals still reference `atp` / `Atp*`. This is a leftover from the rename, not a separate component.

---

## 2. Terminology

| Term | Meaning |
|---|---|
| **token** | In text parsing: a single word from the vector returned by `shell_words.split()`. Elsewhere: a generic reference to either an *instruction* or a *transform*. The overlap is a known source of confusion. |
| **opcode** | Identifies a token in the bytecode-processing context. |
| **transform** | Instruction that modifies the text directly. |
| **instruction** | Logical instruction (variable definitions, blocks, etc.) that does not modify text directly. |
| **block (blk)** | The `blk {x} assoc {token};` instruction. Groups instructions under one name for later reuse. |
| **watcher** | A small function (usually a one-liner) run at some point in the pipeline that returns specific information about the text at that moment. |

> "Instructions" are occasionally called "tokens" in this codebase. Check context to tell which meaning applies.

---

## 3. How It Works

1. **Pipeline** is written (`.tft` file, `.tfb` file, or Rust builder).
2. **Input text** is provided (any file).
3. The pipeline **executes sequentially** over the text.
4. The **transformed text** is written to a new file.

---

## 4. Repository Layout

| Path | Responsibility |
|---|---|
| `src/api` | Sole public entry point when the project is used as a crate. |
| `src/bin` | Project binaries: the CLI today; planned: a REPL shell and an educational CLI game that teaches TextForge. |
| `src/bytecode` | Bytecode compilation. Only present under the `bytecode` feature. |
| `src/context` | The execution context object. A `static_context` with predefined functionality is planned. |
| `src/globals` | The lookup map the engine uses to decide which instruction to call for a given token. **Must be updated whenever an instruction is added.** |
| `src/macros` | Project macros. |
| `src/parser` | Types used for parsing the DSL, and for resolving/converting parameters and variables. |
| `src/text` | Default way to use the DSL; handles the textual form. |
| `src/tokens/instructions` | Logical instructions that do not modify text directly. |
| `src/tokens/transforms` | Instructions that modify text. |
| `src/tokens/**/mod.rs` | Implementation of one specific instruction. |
| `src/tokens/**/test.rs` | Unit tests for that instruction. |
| `src/watchers` | Report-generation machinery used during pipeline processing. |
| `src/utils` | Shared utility definitions. |
| `tests/` | Integration tests and benchmarks. |
| `data/` | Sample files for testing the DSL in different contexts. |
| `pipelines/` | Pipeline files. *(Description incomplete in the original notes — document precisely when confirmed.)* |

---

## 5. Usage Modes

TextForge can be used in three ways.

### 5.1 As a Rust crate

- `TextForgeBuilder` offers a concise builder for creating pipelines (and can give the illusion of recursion/nesting).
- The builder is almost never invoked directly for execution. Use **`TextForgeProcessor`** instead.
- Planned: bindings for other languages via FFI.

**`TextForgeProcessor` processing methods:**

| Method | Behavior |
|---|---|
| `process_single` | Processes one instruction on a given input. |
| `process_all` | Processes one full pipeline on a given input. |
| `process_single_with_debug` | Like `process_single`, and prints processing info to the console. |
| `process_all_with_debug` | Like `process_all`, and prints info for every step. |
| `process_all_with_watchers` | Like `process_all`, with watchers enabled. |
| `process_batch` | Processes multiple pipelines from different files in parallel. |

`TextForgeProcessor` also exposes `process_bytecode` for running compiled pipelines.

### 5.2 As a textual DSL

Hand-written `.tft` files, shareable and processed through the CLI utility **`txf`**.

### 5.3 As a bytecode generator

`.tfb` files describe and share pipelines in an optimized form. Bytecode is always **big-endian** (`be_bytes`). See [Bytecode Format](#8-bytecode-format).

---

## 6. Textual DSL

The textual form resembles assembly, and the comparison is fair in terms of the level of control it gives over the text.

**Syntax rules**

- One instruction per line.
- Every instruction ends with `;`.
- Instructions run sequentially.
- Structure: `{name} {params};`
- Parameters can have different types. Some are pure **literals** that do not affect behavior and are **dropped during parsing** (e.g. `assoc`, `do`).

**Naming convention**

Instruction names (especially transforms) abbreviate a short English phrase describing the operation. Example: `atb` = "add to beginning".

**Execution context**

A `GlobalExecutionContext` is created at the start of each processing run. It can be read and modified during execution and enables variables and other features. Any instruction may use it, though transforms commonly ignore it.

**Errors**

All execution errors are represented by `TextForgeError`. Most methods return a `Result` with that error type.

**Example pipeline**

```text
dlf;            // Delete first character
atb "banana ";  // Add "banana " to the beginning of the input
dll;            // Delete last character
```

Input `xfruitx` → output `banana fruit`.

---

## 7. Interpolation

| Syntax | Purpose | Notes |
|---|---|---|
| `[[ 2 + 3 ]]` | Math expression interpolation | Resolved by `evalexpr`. Variables from the execution context can be referenced inside expressions. |
| `{{name}}` | Variable interpolation | Currently **only parameter substitution** is supported. Replaced at execution time by the value of the named variable. |

---

## 8. Bytecode Format

Enabled by the `bytecode` feature. Compiling to bytecode makes pipeline files faster to read.

**Header**

```text
[magic_number          (8 bytes)]
[protocol_version      (8 bytes)]
[instruction_count     (4 bytes)]
```

**Per instruction** (repeated `instruction_count` times)

```text
[instruction_total_size  (8 bytes)]
[instruction_op_code     (4 bytes)]
[instruction_param_count (1 byte)]
```

**Per parameter** (repeated `instruction_param_count` times)

```text
[param_size          (8 bytes)]
[param_type          (4 bytes)]
[param_payload_size  (4 bytes)]
[param_payload       (N bytes)]
```

**Rules**

- All conversion and reading uses **big-endian only**.
- A file is recognized as TextForge bytecode only if the magic number equals the byte sequence `38, 235, 245, 8, 244, 137, 1, 179`.

---

## 9. Watchers

Watchers are an optional feature (`watchers`). They are small functions that run at arbitrary points in a pipeline and extract specific information from the text at that point (length, whether it contains a given character, etc.).

- Collected results are aggregated and can be **exported to a `.json` report** via `serde` at the end of processing.
- A watcher can inspect: which instruction just ran, the text **before and after** each instruction, and the text's current state.
- **Purpose:** produce detailed, fully customizable feedback about pipelines for the user (human or machine), so logical errors can be found and the pipeline corrected.
- **Not for production runs.** Constantly executing functions adds overhead. Use watchers **only for feedback**. To run a finished pipeline, use `process_all`.

**Key types**

| Type | Role |
|---|---|
| `WatcherContext` | Simple struct exposing the data each watcher can access. Required parameter of every watcher; filled automatically by `process_all_with_watchers` on each pipeline iteration. |
| `WatcherList` | Stores the watchers to be executed. |
| `ExecutionWindows` | Enum specifying *when* a watcher runs (after every instruction, before a specific instruction, after, etc.). |

---

## 10. Core Types

| Type / Macro | Role |
|---|---|
| `InstructionMethods` | Trait implemented by every instruction; defines the universal method contract. |
| `TextForgeError` | Project-wide error type. **Every internal error must be converted to `TextForgeError` before being returned via `Result`.** |
| `TextForgeParamTypes` | Types accepted as parameters by instructions. |
| `ValType` | Simple enum for values that may be variables: either a literal value or a reference to a variable stored in the context. |
| `GlobalExecutionContext` | Manages the execution context of each pipeline. |
| `to_bytecode!` | Macro converting a token to its bytecode. |
| `parse_args!` | Macro converting a `TextForgeParamTypes` into its specific type so it can be stored in the instruction object during execution. |

---

## 11. Cargo Features

| Feature | Effect |
|---|---|
| `watchers` | Enables the reporting mechanism. |
| `bytecode` | Enables bytecode compilation code. |
| `test_access` | Enables **all** features (bytecode + watchers + test helpers). **Use only in tests.** |

---

## 12. Common Tasks

### Add a new instruction

`?` is either `instructions` or `transforms`, depending on the kind of instruction.

1. Create the directory `src/tokens/?/{name}`.
2. Implement the instruction in `src/tokens/?/{name}/mod.rs` using `InstructionMethods`.
3. Add unit tests in `src/tokens/?/{name}/test.rs`.
4. Update the global lookup table in `src/globals/table.rs`. **Avoid collisions** with existing names and opcodes.
5. Update `src/tokens/mod.rs` to include the new instruction. The project handles the rest.
6. Update `build_all_tokens_pipeline_safe` in `src/utils/test_helpers.rs` to include the new instruction.

---

## 13. Golden Rules

### Safety and correctness

1. **No direct `unsafe`** in project code.
2. **Panicking methods are forbidden in main code:** `.unwrap()`, `.expect()`, `.expect_err()`. They are allowed in tests. Propagate every error with `?`.
3. **Unicode-safe.** Every instruction must handle multi-byte characters.
4. **Transforms must handle edge cases:** empty strings, multibyte Unicode, emoji, and combining characters. Cover these in each token's unit tests.

### Testing

5. **All code must be tested.** For an instruction, put unit tests in a `test.rs` next to its `mod.rs`.
6. **Bug fixes must include a test** that reproduces the previous incorrect behavior.
7. **Bytecode changes must include serialization and deserialization tests.**

### Documentation

8. **Every new structure must be documented** with `cargo doc` comments.
9. **Every new object needs a doctest** with a code example, placed immediately before its definition.

### Performance

10. **Instructions must be heavily optimized**; they may run on very large text files.
11. **Optimizations must be justified by benchmarks** and must not introduce odd or unnecessary code.

### Compatibility

12. **Parser changes must preserve compatibility** with existing pipelines unless explicitly authorized.

### Workflow

13. **Each agent works in its own git worktree** to avoid interfering with other agents' work.
14. Run the [pre-commit checklist](#14-pre-commit-checklist) before every commit.
15. Request human approval where required ([section 15](#15-human-approval-required)).

---

## 14. Pre-Commit Checklist

Run all of these before each commit:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features test_access -- -D warnings
cargo test                              # includes doctests
cargo test --features bytecode
cargo test --features watchers
cargo test --features test_access       # enables bytecode + watchers + test helpers
```

---

## 15. Human Approval Required

Ask a human before doing any of the following:

- Publishing to crates.io, bumping the version, or yanking a published version.
- `git push`, force-push, `git reset --hard`, or deleting branches/worktrees that contain unpushed changes.
- Adding a dependency or changing the license.
- Modifying a public contract or renaming file extensions.
- Anything not clearly covered by this document. **When in doubt, leave it out.**

---

## 16. Dependencies

### Runtime

| Crate | Usage |
|---|---|
| `shell-words` | Pre-processing of pipelines in text mode. |
| `serde`, `serde_json` | Watchers; the `jsone` and `jsonu` transforms. |
| `regex` | Regex parsing in several transforms. |
| `urlencoding` | `urle` and `urld` transforms. |
| `html-escape` | `htmle` and `htmlu` transforms. |
| `uuid` | Identifying transforms in the processor. |
| `colored` | Debug-mode output. |
| `clap` | CLI construction. |
| `rayon` | Parallelism in `process_batch`. |
| `evalexpr` | Resolving math expressions in `[[ ... ]]` interpolation. |

### Testing

| Crate | Usage |
|---|---|
| `tempfile` | Temporary files in tests. |
| `random-string` | Random test data. |
| `rand` | Random test data. |


---

## 17. bindings

1. **Independent projects.** Each language binding lives in `src/bindings/{language_name}`. Each directory contains a separate crate, with independent versioning and its own Git repository. Keep binding changes and commits in that repository.

2. **Portable API only.** Bindings must use the intermediate API in `src/api/portable` directly. The binding crate must serve only as an adapter for the target language; do not duplicate the engine, instruction implementations, or domain validation. See [the portable API guide](docs/portable-api.md) and [the processor parity matrix](docs/portable-parity.md).

3. **One-to-one public API parity.** Use the public Rust APIs in `src/api/processor` (`src/api/processor.rs` and its submodules) and `src/api/builder` (`src/api/builder.rs` and its submodules) as the reference surface. Expose a one-to-one functional mirror through the portable API, preserving behavior while adapting signatures, types, and ownership to the target language. If the portable layer lacks a required capability, address that gap there, subject to existing public-contract approval rules, rather than bypassing it in a binding.

4. **Language conventions.** Follow the target language's development conventions, including naming and case styles. For example, Python method names use `snake_case`, while JavaScript method names use `camelCase`. These adaptations must preserve the corresponding Rust API's meaning and behavior.

5. **Core evolution.** Substantial core changes must trigger a review of the corresponding public API, portable API, and bindings. Update them as needed to expose new functionality and preserve parity. Keeping bindings current does not waive the human approval requirement below.

6. **Typing for dynamic languages.** When a dynamically typed target language provides facilities for typing and documenting exposed APIs, use them. Examples include Python type annotations and distributed typing metadata or stubs, and TypeScript declarations for JavaScript bindings.

7. **Human approval.** Updating bindings requires explicit manual approval from a human maintainer before making binding changes.

8. **Native documentation, tests, and commits.** Thoroughly document and test each binding using the target language's native tools and conventions. Make frequent, focused commits for each functionality in the binding's own repository, and run all applicable pre-commit checks before each commit.

9. **Compatibility matrix.** Document the supported core versions or pinned revisions, target-language versions, operating systems, architectures, and enabled Cargo features. Distinguish supported and tested configurations from unverified source-build possibilities. Review this matrix whenever the core snapshot or binding changes.

10. **Errors and lossless conversions.** Preserve portable error codes, stages, messages, and diagnostic context in idiomatic target-language errors. Reject invalid or out-of-range numbers without truncation or silent coercion. Preserve Unicode and native path representations without lossy conversion; document unsupported representations explicitly.

11. **Ownership, callbacks, and concurrency.** Define and document object ownership, resource release, moved/consumed values, callback lifetimes, exception propagation, re-entrancy, and thread-safety. Follow the target runtime's locking requirements. Test supported lifecycle and concurrent-use guarantees, including failure and cleanup paths; mark unavailable facilities explicitly.

12. **Parity tests.** Maintain a method-level mapping between the public Rust API and each binding. Use tests that compare results and errors against the corresponding Rust implementation, including edge cases and enabled optional features. Record missing methods or features as parity gaps; documentation of a gap does not satisfy the one-to-one parity requirement. Keep reference fixtures tied to the supported core revision and detect stale mappings in CI.

13. **Distribution validation.** In CI, build the final distributable packages, install them in clean environments, and run tests against those installed artifacts across the supported compatibility matrix. Verify that typing metadata and required native libraries are included. When source distributions are provided, rebuild and test them outside the checkout. Publishing remains subject to human approval.

14. **Clean, maintainable code.** Apply KISS (Keep It Simple), YAGNI (You Aren't Gonna Need It), DRY (Don't Repeat Yourself), separation of concerns, and single responsibility with human readability and maintenance as the goal. Use descriptive names, small focused functions, explicit control flow, and clear module boundaries. Avoid speculative abstractions, unnecessary indirection, hidden runtime API generation, and duplicated domain logic. Keep generated code readable, identify its source and regeneration command, and verify freshness. Apply these principles pragmatically; do not add abstractions merely to remove trivial repetition. Explain non-obvious interop constraints and tradeoffs in comments.
