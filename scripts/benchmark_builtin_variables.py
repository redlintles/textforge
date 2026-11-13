#!/usr/bin/env python3
"""Benchmark the static context and built-in variables against a baseline.

Builds `examples/builtin_variables_benchmark.rs` in this checkout and,
optionally, in a baseline checkout (the example only uses the 0.2.0 API, so it
can be copied there). Runs both binaries alternately, one process per sample,
and writes the medians to docs/benchmarks/builtin-variables/summary.json.

    python3 scripts/benchmark_builtin_variables.py --baseline ../base --rounds 5
"""
import argparse
import json
import os
import shutil
import statistics
import subprocess
from pathlib import Path

CASES = [
    "small_runs",
    "expression_iter",
    "variable_iter",
    "null_100mb",
    "raw_100mb",
    "lines_100mb",
    "len_chars_100mb",
    "lines_twice_100mb",
]
EXAMPLE = "builtin_variables_benchmark"


def build(checkout: Path, target: Path) -> Path:
    environment = dict(os.environ, CARGO_TARGET_DIR=str(target))
    subprocess.run(
        ["cargo", "build", "-q", "--release", "--example", EXAMPLE],
        cwd=checkout, env=environment, check=True,
    )
    return target / "release" / "examples" / EXAMPLE


def sample(binary: Path, checkout: Path, case: str):
    result = subprocess.run(
        [str(binary), case, "5"], cwd=checkout, capture_output=True, text=True
    )
    if result.returncode != 0:
        return None  # e.g. a built-in variable the baseline does not know
    return json.loads(result.stdout.strip().splitlines()[-1])["median_s"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--target-root", type=Path, default=Path("target/bench-builtins"))
    arguments = parser.parse_args()

    root = Path(__file__).resolve().parent.parent
    trees = {"current": root}
    if arguments.baseline:
        baseline = arguments.baseline.resolve()
        shutil.copy(root / "examples" / f"{EXAMPLE}.rs", baseline / "examples")
        trees["baseline"] = baseline
    binaries = {
        name: build(tree, (arguments.target_root / name).resolve())
        for name, tree in trees.items()
    }

    samples = {name: {case: [] for case in CASES} for name in trees}
    for _ in range(arguments.rounds):
        for case in CASES:
            for name, tree in trees.items():
                value = sample(binaries[name], tree, case)
                if value is not None:
                    samples[name][case].append(value)

    summary = {
        name: {
            case: round(statistics.median(values), 6)
            for case, values in cases.items() if values
        }
        for name, cases in samples.items()
    }
    output = root / "docs" / "benchmarks" / "builtin-variables"
    output.mkdir(parents=True, exist_ok=True)
    (output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
