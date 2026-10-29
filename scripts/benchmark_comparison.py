#!/usr/bin/env python3
"""Linux end-to-end revision comparison; Python stdlib and GNU time only.
Run from any directory: python3 scripts/benchmark_comparison.py [--repetitions 5].
Builds clean commit snapshots, adding the identical worker to both snapshots.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import statistics
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def run(cmd, **kw):
    return subprocess.run(cmd, check=True, **kw)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--report-only', action='store_true', help='regenerate report from existing raw samples')
    p.add_argument('--original', default='608e4fc')
    p.add_argument('--candidate', default='61c089e')
    p.add_argument('--repetitions', type=int, default=5)
    p.add_argument('--iterations', type=int, default=4)
    a = p.parse_args()
    if a.report_only:
        reports = ROOT / 'docs/benchmarks'
        saved = json.loads((reports / 'comparison-results.json').read_text())
        jobs = sorted({(r['scenario'], r['method']) for r in saved['samples']})
        write_report(saved['samples'], reports, jobs)
        return
    if a.repetitions < 2 or a.iterations < 1:
        p.error('use at least two repetitions and one iteration')
    data = ROOT / 'data/benchmark-comparison'
    pipes = ROOT / 'pipelines/benchmark-comparison'
    reports = ROOT / 'docs/benchmarks'
    for directory in (data, pipes, reports):
        directory.mkdir(parents=True, exist_ok=True)
    # Repeat complete records; never split UTF-8 code points.
    record = '2026 INFO user=alice texto=João é 👩🏽‍💻 🇧🇷 banana\n'.encode()
    scenarios = {}
    for name, mib, pipeline in [('wrap', 1, 'atb "<";\nate ">";\n'), ('normalize', 8, 'raw alice "[USER]";\ntua;\natb "<";\nate ">";\n')]:
        inp = data / name / 'input.txt'
        inp.parent.mkdir(exist_ok=True)
        inp.write_bytes(record * ((mib * 1024 ** 2 + len(record) - 1) // len(record)))
        pipe = pipes / (name + '.tft')
        pipe.write_text(pipeline)
        text = inp.read_text()
        if name == 'normalize':
            text = text.replace('alice', '[USER]').upper()
        expected = hashlib.sha256(('<' + text + '>').encode()).hexdigest()
        scenarios[name] = {'input': str(inp.relative_to(ROOT)), 'pipeline': str(pipe.relative_to(ROOT)), 'input_bytes': inp.stat().st_size, 'input_sha256': digest(inp), 'expected_sha256': expected}
    metadata = {'rustc': subprocess.check_output(['rustc', '-Vv'], text=True), 'platform': platform.platform(), 'cpu': Path('/proc/cpuinfo').read_text().split('model name')[1].split('\n')[0].strip(' :\t'), 'logical_cpus': os.cpu_count(), 'rayon_threads': 4, 'iterations': a.iterations, 'repetitions': a.repetitions, 'scenarios': scenarios, 'revisions': {}}
    rows = []
    with tempfile.TemporaryDirectory(prefix='textforge-comparison-') as temp:
        temp = Path(temp)
        binaries = {}
        for label, ref in [('original', a.original), ('candidate', a.candidate)]:
            revision = subprocess.check_output(['git', 'rev-parse', ref], cwd=ROOT, text=True).strip()
            metadata['revisions'][label] = revision
            snapshot = temp / label
            snapshot.mkdir()
            archive = temp / (label + '.tar')
            with archive.open('wb') as f:
                run(['git', 'archive', revision], cwd=ROOT, stdout=f)
            with tarfile.open(archive) as t:
                t.extractall(snapshot, filter='data')
            (snapshot / 'examples').mkdir(exist_ok=True)
            (snapshot / 'examples/comparison_probe.rs').write_bytes((ROOT / 'examples/comparison_probe.rs').read_bytes())
            env = dict(os.environ, CARGO_TARGET_DIR=str(temp / ('target-' + label)))
            run(['cargo', 'build', '--offline', '--release', '--locked', '--example', 'comparison_probe'], cwd=snapshot, env=env)
            binaries[label] = Path(env['CARGO_TARGET_DIR']) / 'release/examples/comparison_probe'
        env = dict(os.environ, RAYON_NUM_THREADS='4', NO_COLOR='1')
        jobs = [(scenario, method) for scenario in scenarios for method in ['process_all', 'process_batch', 'process_all_with_debug']]
        rng = random.Random(20261028)
        for rep in range(-1, a.repetitions):
            rng.shuffle(jobs)
            for scenario, method in jobs:
                order = ['original', 'candidate']
                rng.shuffle(order)
                for label in order:
                    out = temp / 'output'
                    out.mkdir(exist_ok=True)
                    for old in out.glob('result-*.txt'):
                        old.unlink()
                    metrics = temp / 'time.txt'
                    count = 4 if method == 'process_batch' else 1
                    s = scenarios[scenario]
                    iterations = a.iterations * 25 if scenario == 'wrap' else a.iterations
                    cmd = ['/usr/bin/time', '-f', '%e %U %S %M', '-o', str(metrics), str(binaries[label]), method, str(ROOT / s['pipeline']), str(ROOT / s['input']), str(out), str(iterations), str(count)]
                    run(cmd, stdout=subprocess.DEVNULL, env=env, timeout=120)
                    matches = [digest(out / f'result-{i}.txt') == s['expected_sha256'] for i in range(count)]
                    if not all(matches):
                        raise RuntimeError(f'output mismatch: {label} {scenario} {method}')
                    wall, user, system, rss = map(float, metrics.read_text().split())
                    if rep >= 0:
                        rows.append({'scenario': scenario, 'method': method, 'revision': label, 'repetition': rep, 'wall_seconds': wall, 'user_seconds': user, 'system_seconds': system, 'cpu_seconds': user + system, 'cpu_percent': 100 * (user + system) / wall if wall else None, 'peak_rss_kib': int(rss), 'documents': count * iterations, 'sha256_match': True})
                    print(f'{rep=} {label} {scenario} {method}: {wall:.2f}s {rss/1024:.1f}MiB', flush=True)
    (reports / 'comparison-results.json').write_text(json.dumps({'metadata': metadata, 'samples': rows}, indent=2, ensure_ascii=False))
    write_report(rows, reports, jobs)

def write_report(rows, reports, jobs):
    lines = ['# Revision benchmark — Rust 1.99.1', '', 'Identical release workers; default features; locked dependencies; one excluded warm-up per case; randomized paired revision order; four Rayon threads. Five fresh-process samples by default. Each sample includes startup, pipeline parsing, input reads, repeated executions and output writes (100 wrap / 4 normalize executions per worker with default options; exact document counts are in the raw samples). Batch executes four documents per call. Debug stdout goes to /dev/null, so formatting/allocation remain measured but terminal rendering does not.', '', 'CPU = user + system time; CPU% may exceed 100 with parallel execution. RSS is the maximum resident memory of the entire worker, not allocated bytes. GNU time has 10 ms time resolution; small timings are noisy. CPU percentages omit samples whose wall time rounds to zero; nan means no measurable wall time. No claim of statistical significance is made.', '', 'Both revisions use the requested existing APIs. The candidate’s opt-in buffered APIs are separate and are NOT exercised by these three methods; consult docs/tempfile-experiment.md for that distinct experiment.', '', '| Scenario | Method | Version | Wall median (s) | CPU median (s) | CPU% median | RSS median (MiB) | Wall range (s) |', '|---|---|---|---:|---:|---:|---:|---:|']
    for scenario, method in sorted(jobs):
        for label in ['original', 'candidate']:
            group = [r for r in rows if (r['scenario'], r['method'], r['revision']) == (scenario, method, label)]
            def median(key):
                values = [r[key] for r in group if r[key] is not None]
                return statistics.median(values) if values else float('nan')
            lines.append(f"| {scenario} | {method} | {label} | {median('wall_seconds'):.3f} | {median('cpu_seconds'):.3f} | {median('cpu_percent'):.1f} | {median('peak_rss_kib')/1024:.1f} | {min(r['wall_seconds'] for r in group):.2f}–{max(r['wall_seconds'] for r in group):.2f} |")
    lines += ['', 'Every measured output matched an independently computed SHA-256 reference, including every batch output. Input data contain accents, combining marks, emoji and ZWJ sequences.', '', '## Reproduction', '', '`python3 scripts/benchmark_comparison.py`', '', 'Requires Linux, GNU time, Python 3.12+, Rust and cached Cargo dependencies. Input fixtures and textual pipelines live in the repository root under data/benchmark-comparison and pipelines/benchmark-comparison. Generated binaries and output files are temporary. Raw samples, machine metadata, exact revisions and fixture hashes: comparison-results.json.', '', '## Compiler regression scope', '', 'Only Rust 1.99.1 is installed. Tests on this compiler can reveal failures, but cannot establish that an issue was introduced by the upgrade without running the same commit on the previous compiler. See validation-results.json for the feature/check matrix.']
    (reports / 'comparison-report.md').write_text('\n'.join(lines) + '\n')

if __name__ == '__main__':
    main()
