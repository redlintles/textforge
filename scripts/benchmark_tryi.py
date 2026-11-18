#!/usr/bin/env python3
"""Benchmark of the 0.4.0 error work: what `tryi` costs, and that pipelines
without it are not slower than before.

Usage:
    python3 scripts/benchmark_tryi.py --before OLD_TXF --after NEW_TXF [--runs 7] [--out FILE]

OLD_TXF is a release `txf` built from the revision before the change (it has
no `tryi`, so only the workloads without it run there), NEW_TXF one built from
the revision with it. Each workload runs `--runs` times; the median wall time
is reported. Results are printed as a Markdown table and, with --out, written
as JSON (docs/benchmarks/tryi/).
"""
import argparse
import json
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def variables(count):
    return ''.join(f'var v{i} = x;\n' for i in range(count))


# (name, pipeline, input, uses tryi)
def workloads(directory):
    big = directory / 'big.txt'
    big.write_text(('The quick brown fox jumps over the lazy dog. ação 🍌\n' * 400_000))
    small = directory / 'small.txt'
    small.write_text('abc')
    loops = 3_000_000
    rows = [
        ('20 MiB, five instructions', "tua;\nraw a b;\ntbs;\nrev;\nurle;\n", big, False),
        (f'{loops:,} iterations of tua', f"blk b assoc iter {loops} times tua;\ncblk b;\n", small, False),
        (f'{loops:,} iterations of tryi tua (succeeds)', f"blk b assoc iter {loops} times tryi tua;\ncblk b;\n", small, True),
        (f'{loops:,} iterations of tryi dls 99 (fails, ignored)', f"blk b assoc iter {loops} times tryi dls 99;\ncblk b;\n", small, True),
    ]
    blocks = 1_000_000
    for count in (0, 10, 1000):
        body = variables(count) + 'blk c assoc tua;\n'
        rows.append((f'{blocks:,} × cblk, {count} variables', body + f"blk b assoc iter {blocks} times cblk c;\ncblk b;\n", small, False))
        rows.append((f'{blocks:,} × tryi cblk, {count} variables', body + f"blk b assoc iter {blocks} times tryi cblk c;\ncblk b;\n", small, True))
    return rows


def measure(binary, pipeline, text, runs):
    times = []
    for _ in range(runs):
        start = time.perf_counter()
        result = subprocess.run([binary, '--no-rc', '-f', str(pipeline), '-i', str(text)],
                                stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        times.append(time.perf_counter() - start)
        if result.returncode != 0:
            raise SystemExit(f'{binary} failed on {pipeline}: {result.stderr.decode()}')
    return statistics.median(times)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--before', required=True)
    parser.add_argument('--after', required=True)
    parser.add_argument('--runs', type=int, default=7)
    parser.add_argument('--out')
    arguments = parser.parse_args()
    results = []
    with tempfile.TemporaryDirectory() as name:
        directory = Path(name)
        for index, (label, source, text, uses_tryi) in enumerate(workloads(directory)):
            pipeline = directory / f'p{index}.textforge'
            pipeline.write_text(source)
            before = None if uses_tryi else measure(arguments.before, pipeline, text, arguments.runs)
            after = measure(arguments.after, pipeline, text, arguments.runs)
            results.append({'workload': label, 'before_s': before, 'after_s': after})
            print(f'{label}: before {before}, after {after}', flush=True)
    print('\n| Workload | Before (s) | After (s) | Change |\n|---|---|---|---|')
    for row in results:
        before, after = row['before_s'], row['after_s']
        change = f'{(after / before - 1) * 100:+.1f}%' if before else '—'
        shown = f'{before:.3f}' if before else '—'
        print(f"| {row['workload']} | {shown} | {after:.3f} | {change} |")
    if arguments.out:
        Path(arguments.out).write_text(json.dumps(results, indent=2) + '\n')


if __name__ == '__main__':
    main()
