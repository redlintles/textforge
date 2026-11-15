#!/usr/bin/env python3
"""Benchmark the .textforge line parser against shell_words::split.

Runs the ignored test `benchmark_against_shell_words` (src/text/line_parser/tests.rs)
in release mode and prints its result line. Results: docs/benchmarks/line-parser/.
"""
import subprocess
import sys

cmd = [
    "cargo", "test", "--release", "--features", "test_access", "--lib",
    "benchmark_against_shell_words", "--", "--ignored", "--nocapture",
]
out = subprocess.run(cmd, capture_output=True, text=True)
lines = [l for l in out.stdout.splitlines() if l.startswith("lines=")]
if out.returncode != 0 or not lines:
    sys.stderr.write(out.stdout + out.stderr)
    sys.exit(1)
print(lines[0])
