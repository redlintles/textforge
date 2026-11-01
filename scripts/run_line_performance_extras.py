"""Targeted follow-up: reverse-index tradeoffs, retained RAM and deletion chains.

Run after run_line_performance_audit.py with the same --binary argument.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import statistics
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs/benchmarks/line-performance"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    args = parser.parse_args()
    cpu = min(os.sched_getaffinity(0))
    jobs = []
    for kind, size in [
        ("ascii", 64000),
        ("ascii", 8 * 1024**2),
        ("wide", 64 * 1024**2),
        ("blank", 8 * 1024**2),
    ]:
        for variant, case in [
            ("baseline", "revln"),
            ("prototype", "revln"),
            ("prototype", "revln_offsets"),
        ]:
            jobs.append((variant, case, kind, size, "borrowed"))
    for mode in ["borrowed", "owned"]:
        for variant, case in [
            ("baseline", "slnc"),
            ("prototype", "slnc"),
            ("prototype", "select_exact"),
        ]:
            jobs.append((variant, case, "wide", 64 * 1024**2, mode))
    for size in [64000, 8 * 1024**2]:
        for steps in [10, 50]:
            jobs.extend(
                [
                    (variant, str(steps), "ascii", size, "borrowed")
                    for variant in ["chain_base", "chain_proto"]
                ]
            )
            jobs.append(("baseline", f"batch_{steps}", "ascii", size, "borrowed"))
    work = [(key, sample) for key in jobs for sample in range(5)]
    random.Random(20261031).shuffle(work)
    rows = []
    for index, (key, sample) in enumerate(work):
        variant, case, kind, size, mode = key
        iterations = 20 if size < 100000 else 1
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
        rows.append(row)
        if index % 25 == 0:
            print("extras timing", index, "/", len(work), flush=True)
    (OUT / "extras-timings-raw.json").write_text(json.dumps(rows, indent=2) + "\n")
    summaries = []
    for variant, case, kind, size, mode in jobs:
        selected = [
            r
            for r in rows
            if (r["variant"], r["case"], r["dataset"], r["bytes"], r["ownership"])
            == (variant, case, kind, size, mode)
        ]
        values = [r["ns_per_op"] for r in selected]
        q = statistics.quantiles(values, n=4, method="inclusive")
        row = dict(selected[0])
        row.pop("sample")
        row.pop("ns_per_op")
        row.update(
            median_ns=statistics.median(values),
            p25_ns=q[0],
            p75_ns=q[2],
            min_ns=min(values),
            max_ns=max(values),
        )
        summaries.append(row)
    (OUT / "extras-timings.json").write_text(json.dumps(summaries, indent=2) + "\n")
    rss = []
    for variant, case, kind, size, mode in [
        ("baseline", "revln", "blank", 8 * 1024**2, "borrowed"),
        ("prototype", "revln", "blank", 8 * 1024**2, "borrowed"),
        ("prototype", "revln_offsets", "blank", 8 * 1024**2, "borrowed"),
        ("baseline", "slnc", "wide", 64 * 1024**2, "owned"),
        ("prototype", "slnc", "wide", 64 * 1024**2, "owned"),
        ("prototype", "select_exact", "wide", 64 * 1024**2, "owned"),
        ("baseline", "nln", "blank", 8 * 1024**2, "owned"),
        ("prototype", "nln", "blank", 8 * 1024**2, "owned"),
    ]:
        for sample in range(3):
            path = OUT / "rss-extra-current.txt"
            output = subprocess.check_output(
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
            row = json.loads(output)
            row.update(max_rss_kib=int(path.read_text().strip()), sample=sample)
            rss.append(row)
    (OUT / "extras-rss.json").write_text(json.dumps(rss, indent=2) + "\n")
    (OUT / "rss-extra-current.txt").unlink()
    metadata = {
        "source_sha256": hashlib.sha256(
            (ROOT / "examples/line_performance_audit.rs").read_bytes()
        ).hexdigest(),
        "binary_sha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(),
        "line_descriptor_bytes": 32,
        "offset_descriptor_bytes": 8,
        "samples": 5,
        "cpu_affinity": cpu,
        "note": "same audited production revision, additional prototypes; original kernels unchanged; current RSS from /proc/self/smaps_rollup, peak RSS from GNU time",
    }
    (OUT / "extras-environment.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print("extras done", flush=True)


if __name__ == "__main__":
    main()
