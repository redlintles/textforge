#!/usr/bin/env python3
"""Compare two engine_benchmark executables case by case.

Build each revision with
`cargo build --release --features bytecode --example engine_benchmark`
and pass the two binaries. Each case runs in fresh processes, alternating the
binaries, pinned to one CPU; the median of the per-process medians is reported.
Run from the repository root (the raw_100mb and batch cases read `data/`).
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess

CASES = [
    'parse_text_5k', 'load_bytecode_5k', 'small_runs', 'long_literal_pipeline',
    'variable_params', 'template_params', 'expression_params', 'iter_expression',
    'iter_literal', 'blocks', 'variables_churn', 'raw_100mb', 'batch_64x1mb',
]


def run(binary, case, samples):
    output = subprocess.check_output([str(binary), case, str(samples)], text=True)
    return json.loads(output.strip().splitlines()[-1])['median_s']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--rounds', type=int, default=3)
    parser.add_argument('--samples', type=int, default=5)
    parser.add_argument('--cases', nargs='+', default=CASES)
    args = parser.parse_args()
    if min(args.rounds, args.samples) < 1:
        parser.error('rounds and samples must be positive')
    binaries = {'baseline': args.baseline.resolve(), 'candidate': args.candidate.resolve()}
    cpu = min(os.sched_getaffinity(0))
    os.sched_setaffinity(0, {cpu})
    result = {
        'metadata': {
            'platform': platform.platform(), 'cpu': cpu,
            'cpu_model': next((line.split(':', 1)[1].strip()
                               for line in Path('/proc/cpuinfo').read_text().splitlines()
                               if line.startswith('model name')), 'unknown'),
            'rustc': subprocess.check_output(['rustc', '-V'], text=True).strip(),
            'rounds': args.rounds, 'samples_per_process': args.samples,
            'binaries_sha256': {name: hashlib.sha256(path.read_bytes()).hexdigest()
                                for name, path in binaries.items()},
            'note': 'Fresh processes, alternating order, one warm-up per binary and case.',
        },
        'summary': [],
    }
    for case in args.cases:
        times = {name: [] for name in binaries}
        for name, binary in binaries.items():
            run(binary, case, 1)
        for round_index in range(args.rounds):
            order = list(binaries.items())
            if round_index % 2:
                order.reverse()
            for name, binary in order:
                times[name].append(run(binary, case, args.samples))
        baseline = statistics.median(times['baseline'])
        candidate = statistics.median(times['candidate'])
        row = {'case': case, 'baseline_s': baseline, 'candidate_s': candidate,
               'speedup': baseline / candidate if candidate else None, 'runs': times}
        result['summary'].append(row)
        print(f"{case:24} {baseline:10.6f} {candidate:10.6f} {row['speedup']:7.2f}x", flush=True)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
