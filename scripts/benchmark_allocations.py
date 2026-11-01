#!/usr/bin/env python3
"""Compare identically built allocation_benchmark executables using GNU time."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--candidate', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--rounds', type=int, default=5)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--sizes-mib', type=int, nargs='+', default=[4, 16])
    args = parser.parse_args()
    if min(args.rounds, args.repeats, *args.sizes_mib) < 1:
        parser.error('rounds, repeats and sizes must be positive')
    binaries = {'baseline': args.baseline.resolve(), 'candidate': args.candidate.resolve()}
    cpu = min(os.sched_getaffinity(0))
    os.sched_setaffinity(0, {cpu})
    row = 'banana pear apple é🍌 e\u0301 ß ΟΣ İ <tag>& value\r\nbanana duplicate\r\n\r\nlast line\n'.encode()
    result = {'metadata': {'platform': platform.platform(), 'cpu': cpu,
                          'cpu_model': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')), 'unknown'),
                          'rustc': subprocess.check_output(['rustc', '-Vv'], text=True),
                          'rounds': args.rounds, 'repeats': args.repeats,
                          'baseline_revision': '72fc7fc',
                          'binaries_sha256': {name: hashlib.sha256(path.read_bytes()).hexdigest() for name, path in binaries.items()},
                          'harness_sha256': hashlib.sha256((Path(__file__).resolve().parent.parent / 'examples/allocation_benchmark.rs').read_bytes()).hexdigest(),
                          'features': ['test_access'], 'execution': 'registered run_transform (in-memory Cow path)',
                          'note': 'Fresh sequential processes; one warm-up per binary/scenario; alternating order; GNU time includes startup, input read, setup and output digest.'},
              'corpora': [], 'samples': [], 'summary': []}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='textforge-allocation-bench-') as directory:
        directory = Path(directory)
        for size in args.sizes_mib:
            corpus = row * ((size * 1024 * 1024) // len(row))
            input_path = directory / f'{size}MiB.txt'
            input_path.write_bytes(corpus)
            result['corpora'].append({'size_mib': size, 'bytes': len(corpus), 'sha256': hashlib.sha256(corpus).hexdigest()})
            for mode in ['coverage', 'large']:
                expected = None
                for iteration in range(-1, args.rounds):
                    order = ['baseline', 'candidate'] if iteration % 2 == 0 else ['candidate', 'baseline']
                    for version in order:
                        metrics = directory / 'time.json'
                        command = ['/usr/bin/time', '-f', '{"wall_seconds":%e,"user_seconds":%U,"system_seconds":%S,"max_rss_kib":%M}', '-o', str(metrics), str(binaries[version]), mode, str(input_path), str(args.repeats)]
                        process = subprocess.run(command, text=True, capture_output=True, check=True)
                        sample = json.loads(process.stdout)
                        identity = (sample['digest'], sample['output_bytes'], sample['pipelines'], sample['input_bytes'], sample['repeats'])
                        if expected is None:
                            expected = identity
                        if expected != identity:
                            raise RuntimeError(f'Output/workload mismatch: {version} {mode} {size}MiB')
                        sample.update(json.loads(metrics.read_text()))
                        sample.update(version=version, size_mib=size, round=iteration)
                        sample['cpu_seconds'] = sample['user_seconds'] + sample['system_seconds']
                        sample['cpu_percent'] = 100 * sample['cpu_seconds'] / sample['wall_seconds']
                        if iteration >= 0:
                            result['samples'].append(sample)
                            args.output.write_text(json.dumps(result, indent=2) + '\n')
                            print(f'{size}MiB {mode} round={iteration + 1} {version}: wall={sample["wall_seconds"]:.2f}s CPU={sample["cpu_seconds"]:.2f}s RSS={sample["max_rss_kib"]/1024:.1f}MiB', flush=True)
                for version in binaries:
                    samples = [s for s in result['samples'] if (s['mode'], s['size_mib'], s['version']) == (mode, size, version)]
                    summary = {'mode': mode, 'size_mib': size, 'version': version}
                    for metric in ['wall_seconds', 'elapsed_seconds', 'cpu_seconds', 'cpu_percent', 'max_rss_kib']:
                        values = [s[metric] for s in samples]
                        summary[metric] = {'median': statistics.median(values), 'min': min(values), 'max': max(values)}
                    result['summary'].append(summary)
                args.output.write_text(json.dumps(result, indent=2) + '\n')


if __name__ == '__main__':
    main()
