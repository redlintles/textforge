#!/usr/bin/env python3
"""Compare two `txf` binaries on the same inputs: time, peak memory, output.

Usage:
  scripts/benchmark_cli.py --base OLD_TXF --new NEW_TXF [--sizes 1,50,200]
                           [--runs 3] [--timeout 300] [--json results.json]

Both binaries run each case on generated text (log-like lines, about 10% of
them with multibyte characters) of each size in MiB, writing to a file. For
every case the script reports the median wall time, the peak resident memory
(ru_maxrss of the child) and whether both outputs are byte-identical. A run
that exceeds --timeout is stopped and reported as such. Results and the
method: docs/benchmarks/cli/.
"""
import argparse
import hashlib
import json
import os
import platform
import random
import statistics
import subprocess
import sys
import tempfile
import time

PIPELINES = {
    "upper": "tua;\n",
    "squash": 'raw "\\s+" " ";\n',
    "trim-mark": "tbs;\nate \";\";\n",
}

CASES = [
    ("upper", "all"),
    ("squash", "all"),
    ("upper", "line"),
    ("trim-mark", "line"),
    ("upper", "chunk-65536"),
    ("upper", "chunk-4096"),
]


def make_input(path, mebibytes):
    rng = random.Random(42)
    words = ["error", "warn", "info", "user", "request", "timeout", "ação", "naïve", "日本", "🙂"]
    target = mebibytes * 1024 * 1024
    with open(path, "w", encoding="utf-8", newline="") as out:
        written = 0
        while written < target:
            ascii_only = rng.random() < 0.9
            pool = words[:6] if ascii_only else words
            line = "  " + " ".join(rng.choice(pool) for _ in range(rng.randint(3, 14))) + "   \n"
            out.write(line)
            written += len(line.encode("utf-8"))


def run_once(binary, pipeline, mode, source, target, timeout):
    command = [binary, "--no-rc", "-f", pipeline, "-i", source, "-o", target, "-r", mode]
    start = time.perf_counter()
    process = subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    deadline = start + timeout
    while True:
        pid, status, usage = os.wait4(process.pid, os.WNOHANG)
        if pid:
            elapsed = time.perf_counter() - start
            if os.waitstatus_to_exitcode(status) != 0:
                raise RuntimeError(f"{command} failed: {process.stderr.read().decode()}")
            return elapsed, usage.ru_maxrss // 1024
        if time.perf_counter() > deadline:
            process.kill()
            os.wait4(process.pid, 0)
            return None, None
        time.sleep(0.005)


def digest(path):
    hasher = hashlib.sha256()
    with open(path, "rb") as file:
        for block in iter(lambda: file.read(1 << 20), b""):
            hasher.update(block)
    return hasher.hexdigest()


def measure(binary, pipeline, mode, source, target, runs, timeout):
    times, memory = [], []
    for _ in range(runs):
        elapsed, rss = run_once(binary, pipeline, mode, source, target, timeout)
        if elapsed is None:
            return {"timeout": timeout}, None
        times.append(elapsed)
        memory.append(rss)
    return {"median_s": statistics.median(times), "min_s": min(times), "peak_mib": max(memory)}, digest(target)


def startup(binary, pipeline, directory, runs=50):
    source = os.path.join(directory, "tiny.txt")
    with open(source, "w") as file:
        file.write("hello world\n")
    times = []
    for _ in range(runs):
        elapsed, _ = run_once(binary, pipeline, "all", source, os.path.join(directory, "tiny.out"), 10)
        times.append(elapsed)
    return statistics.median(times)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--base", required=True)
    parser.add_argument("--new", required=True)
    parser.add_argument("--sizes", default="1,50,200")
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--cases", default=",".join(f"{p}:{m}" for p, m in CASES))
    parser.add_argument("--json")
    args = parser.parse_args()
    cases = [tuple(case.split(":")) for case in args.cases.split(",")]

    results = {
        "machine": {"cpu": platform.processor() or platform.machine(), "cpus": os.cpu_count(), "python": platform.python_version()},
        "base": args.base, "new": args.new, "runs": args.runs, "cases": [],
    }
    with tempfile.TemporaryDirectory() as directory:
        paths = {}
        for name, text in PIPELINES.items():
            paths[name] = os.path.join(directory, f"{name}.textforge")
            with open(paths[name], "w") as file:
                file.write(text)
        results["startup_ms"] = {
            "base": startup(args.base, paths["upper"], directory) * 1000,
            "new": startup(args.new, paths["upper"], directory) * 1000,
        }
        print(f"startup (tiny input, median of 50): base {results['startup_ms']['base']:.2f} ms, new {results['startup_ms']['new']:.2f} ms", flush=True)
        for size in [int(s) for s in args.sizes.split(",")]:
            source = os.path.join(directory, f"input-{size}.txt")
            make_input(source, size)
            for pipeline, mode in cases:
                row = {"size_mib": size, "pipeline": pipeline, "mode": mode}
                outputs = {}
                for label in ("base", "new"):
                    target = os.path.join(directory, f"out-{label}.txt")
                    row[label], outputs[label] = measure(
                        getattr(args, label), paths[pipeline], mode, source, target, args.runs, args.timeout
                    )
                row["same_output"] = None if None in outputs.values() else outputs["base"] == outputs["new"]
                results["cases"].append(row)
                fmt = lambda r: f"timeout >{r['timeout']:.0f}s" if "timeout" in r else f"{r['median_s']:.3f}s {r['peak_mib']}MiB"
                print(f"{size:>4} MiB {pipeline:<10} {mode:<12} base {fmt(row['base']):<22} new {fmt(row['new']):<22} same={row['same_output']}", flush=True)
            os.remove(source)
    if args.json:
        with open(args.json, "w") as file:
            json.dump(results, file, indent=2)
    if any(row["same_output"] is False for row in results["cases"]):
        sys.exit("outputs differ")


if __name__ == "__main__":
    main()
