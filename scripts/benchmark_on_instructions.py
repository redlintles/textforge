"""Benchmark `onln`/`onck`/`onlni` against whole-document instructions.

Build first: cargo build --release --example on_instructions_benchmark
Run: python3 scripts/benchmark_on_instructions.py --binary target/release/examples/on_instructions_benchmark

The ~100 MB input repeats data/book1.txt, book2.txt and book3.txt. Every sample
runs in a fresh process; peak RSS is the child's VmHWM. Standard library only.
"""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/benchmarks/on-instructions"
CASES = [
    "identity",
    "raw_whole",
    "onlni_whole",
    "onlni_tenth",
    "onln_middle",
    "onln_ten",
    "onck_middle",
]


def build_input(path: Path, target_bytes: int) -> None:
    seed = "".join(
        (ROOT / "data" / name).read_text(encoding="utf-8")
        for name in ("book1.txt", "book2.txt", "book3.txt")
    )
    repeats = max(1, target_bytes // len(seed.encode("utf-8")))
    path.write_text(seed * repeats, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--megabytes", type=int, default=100)
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    raw = {case: [] for case in CASES}
    with tempfile.TemporaryDirectory() as work:
        work_dir = Path(work)
        input_path = work_dir / "input.txt"
        build_input(input_path, args.megabytes * 1024 * 1024)
        for _ in range(args.samples):
            for case in CASES:
                result = subprocess.run(
                    [str(args.binary.resolve()), case, str(input_path), str(work_dir)],
                    check=True,
                    capture_output=True,
                    text=True,
                )
                raw[case].append(json.loads(result.stdout))
    summary = {
        case: {
            "bytes": samples[0]["bytes"],
            "lines": samples[0]["lines"],
            "output_bytes": samples[0]["output_bytes"],
            "median_seconds": statistics.median(s["seconds"] for s in samples),
            "min_seconds": min(s["seconds"] for s in samples),
            "max_seconds": max(s["seconds"] for s in samples),
            "median_peak_rss_mib": statistics.median(s["peak_rss_kib"] for s in samples) / 1024,
        }
        for case, samples in raw.items()
    }
    environment = {
        "revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "rustc": subprocess.check_output(["rustc", "-V"], text=True).strip(),
        "platform": platform.platform(),
        "cpu_count": __import__("os").cpu_count(),
        "samples": args.samples,
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "method": "release build, default features, process_all; one fresh process per sample; "
        "input reading and pipeline parsing excluded from seconds; peak RSS includes the input",
    }
    (OUT / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    (OUT / "raw.json").write_text(json.dumps(raw, indent=2) + "\n")
    (OUT / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    for case, values in summary.items():
        print(
            f"{case:12} {values['median_seconds']:.4f}s "
            f"(min {values['min_seconds']:.4f}, max {values['max_seconds']:.4f}) "
            f"rss {values['median_peak_rss_mib']:.1f} MiB"
        )


if __name__ == "__main__":
    main()
