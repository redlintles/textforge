"""Run deterministic, process-isolated Linux line-transform audit measurements.

Build first: cargo build --release --locked --example line_performance_audit
Run: python3 scripts/run_line_performance_audit.py --binary /absolute/path/to/example
Only standard-library modules and /usr/bin/time are used. No allocator hooks or unsafe.
"""

from __future__ import annotations

import argparse
import json
import os
import platform
import random
import statistics
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/benchmarks/line-performance"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--samples", type=int, default=5)
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    cpu = min(os.sched_getaffinity(0))
    meta = {
        "audited_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "rustc": subprocess.check_output(["rustc", "-Vv"], text=True),
        "platform": platform.platform(),
        "cpu_affinity": cpu,
        "cpu": subprocess.check_output(["lscpu"], text=True),
        "samples": args.samples,
        "seed": 20261031,
        "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "method": "release, one child per sample, input construction and owned acquisition excluded from transform timing; output drop excluded; fresh-process GNU time max RSS includes fixture construction",
    }
    (OUT / "environment.json").write_text(json.dumps(meta, indent=2) + "\n")
    base = [
        "apl",
        "dln",
        "dlna",
        "dlnb",
        "slnc",
        "dlni",
        "inl",
        "nln",
        "rln",
        "pln",
        "dle",
        "klm",
        "dlm",
        "revln",
        "uniqln",
        "joinln",
    ]
    candidates = [
        "apl",
        "dln",
        "dln_last",
        "dlna",
        "dlna_last",
        "slnc",
        "slnc_all",
        "inl_end",
        "nln",
        "revln",
    ]
    datasets = [
        ("ascii", 64000),
        ("ascii", 8 * 1024**2),
        ("wide", 64 * 1024**2),
        ("unicode", 8 * 1024**2),
    ]
    keys = set()
    for kind, size in datasets:
        for case in base:
            keys.add(("baseline", case, kind, size, "borrowed"))
        for case in candidates:
            for mode in ["borrowed", "owned", "spare"]:
                for variant in ["baseline", "prototype"]:
                    keys.add((variant, case, kind, size, mode))
        for variant in ["scan_chars", "scan_bytes"]:
            keys.add((variant, "scanner", kind, size, "borrowed"))
    for kind in ["single", "repeat"]:
        for case in ["apl", "nln", "revln"]:
            for mode in ["borrowed", "owned"]:
                for variant in ["baseline", "prototype"]:
                    keys.add((variant, case, kind, 8 * 1024**2, mode))
        for case in ["dle", "klm_all", "dlm_none", "uniqln", "joinln"]:
            keys.add(("baseline", case, kind, 8 * 1024**2, "borrowed"))
    jobs = [(key, sample) for key in sorted(keys) for sample in range(args.samples)]
    random.Random(20261031).shuffle(jobs)
    raw = []
    started = time.monotonic()
    for index, (key, sample) in enumerate(jobs):
        variant, case, kind, size, mode = key
        iterations = 100 if size < 100000 else (2 if size < 16 * 1024**2 else 1)
        result = subprocess.check_output(
            [
                "taskset",
                "-c",
                str(cpu),
                str(args.binary),
                variant,
                case,
                kind,
                str(size),
                mode,
                str(iterations),
                "timing",
            ],
            text=True,
        )
        row = json.loads(result)
        row["sample"] = sample
        raw.append(row)
        if index % 100 == 0:
            print(
                f"timing {index}/{len(jobs)} ({time.monotonic() - started:.1f}s)",
                flush=True,
            )
    (OUT / "timings-raw.json").write_text(json.dumps(raw, indent=2) + "\n")
    summary = []
    for key in sorted(keys):
        variant, case, kind, size, mode = key
        values = [
            row
            for row in raw
            if (row["variant"], row["case"], row["dataset"], row["ownership"])
            == (variant, case, kind, mode)
            and row["bytes"] <= size
            and (row["bytes"] > 100000) == (size > 100000)
        ]
        # The two ascii sizes are separated explicitly; all generators emit <= requested bytes.
        assert len(values) == args.samples, (key, len(values))
        times = [row["ns_per_op"] for row in values]
        q = statistics.quantiles(times, n=4, method="inclusive")
        row = dict(values[0])
        row.pop("sample")
        row.pop("ns_per_op")
        row.update(
            median_ns=statistics.median(times),
            min_ns=min(times),
            max_ns=max(times),
            p25_ns=q[0],
            p75_ns=q[2],
        )
        summary.append(row)
    (OUT / "timings.json").write_text(json.dumps(summary, indent=2) + "\n")
    memory_jobs = []
    for kind, size in [
        ("ascii", 64 * 1024**2),
        ("blank", 8 * 1024**2),
        ("ascii", 8 * 1024**2),
    ]:
        memory_jobs.append(("prototype", "input_only", kind, size, "owned"))
        for case in ["dln", "slnc", "apl", "revln", "uniqln"]:
            for mode in ["borrowed", "owned"]:
                for variant in ["baseline", "prototype"]:
                    if variant == "prototype" and case == "uniqln":
                        continue
                    memory_jobs.append((variant, case, kind, size, mode))
    memory = []
    for index, key in enumerate(memory_jobs):
        variant, case, kind, size, mode = key
        for sample in range(3):
            path = OUT / "rss-current.txt"
            result = subprocess.check_output(
                [
                    "/usr/bin/time",
                    "-f",
                    "%M",
                    "-o",
                    str(path),
                    "taskset",
                    "-c",
                    str(cpu),
                    str(args.binary),
                    variant,
                    case,
                    kind,
                    str(size),
                    mode,
                    "1",
                    "rss",
                ],
                text=True,
            )
            row = json.loads(result)
            row.update(max_rss_kib=int(path.read_text().strip()), sample=sample)
            memory.append(row)
        print(f"rss {index + 1}/{len(memory_jobs)}", flush=True)
    (OUT / "rss.json").write_text(json.dumps(memory, indent=2) + "\n")
    (OUT / "rss-current.txt").unlink()
    print(
        f"completed {len(raw)} timing samples and {len(memory)} RSS samples in {time.monotonic() - started:.1f}s",
        flush=True,
    )


if __name__ == "__main__":
    main()
