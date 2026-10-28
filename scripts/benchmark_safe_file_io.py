#!/usr/bin/env python3
"""Measure safe file I/O against the complete pre-removal source snapshot."""
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import statistics
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / 'docs/benchmarks/safe-file-io'
REPETITIONS = 5


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def prepare_data():
    data = ROOT / 'data/safe-file-io'
    pipelines = ROOT / 'pipelines/safe-file-io'
    data.mkdir(parents=True, exist_ok=True)
    pipelines.mkdir(parents=True, exist_ok=True)
    record = '2026 INFO user=alice texto=João é 👩🏽‍💻 🇧🇷 banana\n'
    records = (16 * 1024**2 + len(record.encode()) - 1) // len(record.encode())
    content = (record * records).encode()
    for index in range(4):
        (data / f'input-{index}.txt').write_bytes(content)
    (pipelines / 'read-copy.tft').write_text('null;\n')
    operations = ['atb "<";\n', 'ate ">";\n', 'raw alice "[USER]";\n', 'tua;\n']
    (pipelines / 'mixed-64.tft').write_text(''.join(operations * 16))
    transformed = record
    for _ in range(16):
        transformed = transformed.replace('alice', '[USER]').upper()
    expected = {
        'read-copy': hashlib.sha256(content).hexdigest(),
        'mixed-64': hashlib.sha256(('<' * 16 + transformed * records + '>' * 16).encode()).hexdigest(),
    }
    return data, pipelines, expected


def write_report(saved):
    lines = [
        '# Safe whole-document file I/O', '',
        '## Result summary', '',
        'Before uses the existing mmap paths. After removes both mappings and the memmap2 dependency, reads through File::read_to_end, validates UTF-8 without a second byte copy and transfers the owned input into the non-buffered engine. Pipeline preparation and all earlier optimizations are identical in both snapshots.', '',
        '## Measurements', '',
        '| Workload | Method | Workers | Version | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |',
        '|---|---|---:|---|---:|---:|---:|---|',
    ]
    summaries = []
    for workload in ['read-copy', 'mixed-64']:
        for method, threads in [('file', 1), ('batch', 1), ('batch', 4)]:
            medians = {}
            for version in ['before', 'after']:
                rows = [row for row in saved['samples'] if (row['workload'], row['method'], row['threads'], row['version']) == (workload, method, threads, version)]
                medians[version] = {key: statistics.median(row[key] for row in rows) for key in ['wall_seconds', 'cpu_seconds', 'peak_rss_kib']}
                value = medians[version]
                low = min(row['wall_seconds'] for row in rows)
                high = max(row['wall_seconds'] for row in rows)
                lines.append(f"| {workload} | {method} | {threads} | {version} | {value['wall_seconds']:.2f} | {value['cpu_seconds']:.2f} | {value['peak_rss_kib']/1024:.1f} | {low:.2f}–{high:.2f} |")
            changes = {key: 100 * (medians['after'][key] / medians['before'][key] - 1) for key in medians['before']}
            summaries.append(f"{workload}, {method}, {threads} worker(s): wall {changes['wall_seconds']:+.1f}%, CPU {changes['cpu_seconds']:+.1f}%, peak RSS {changes['peak_rss_kib']:+.1f}%.")
    lines[4:4] = [*summaries, '']
    lines += [
        '', '## Methodology', '',
        f"{REPETITIONS} fresh-process samples per version/configuration ({len(saved['samples'])} samples), randomized paired order with fixed seed and one excluded paired warm-up. Same release worker, default features, locked offline dependencies, {saved['metadata']['rust']}. Four distinct files contain identical Unicode records, each {saved['metadata']['bytes_per_file']} bytes. read-copy runs null through eight batches or eight sequential groups of four files (512 MiB read/written); mixed-64 runs one group, 64 instructions per document: prefix, suffix, regex replacement and Unicode uppercase. File mode is sequential; batch uses a local Rayon pool created once per worker and reused across iterations. Both versions use the same pool.install + process_batch call sequence; process_batch_in_pool provides this sequence as a convenience and is separately tested from inside an executing instruction.", '',
        'GNU time records wall time, user+system CPU time and peak RSS. Startup, pool creation, pipeline registration, reads, transforms and writes are included; compilation, Python reference generation and SHA-256 checks are excluded. Cache is warm, no fsync is requested, and timer resolution is 10 ms. RSS excludes unmapped filesystem page cache; resident file mappings are reclaimable and their removal does not imply the same reduction in system-wide physical memory. These workloads do not establish cold-disk throughput or statistical significance. Every output file from every timed/warm-up run matches an independent reference.', '',
        'The buffered batch already did not use mmap, so it is not relabeled as a removal gain. process_file with buffered enabled still uses its existing buffered process_all dispatch after a safe read. The explicit process_file_buffered API remains the path for initial spooling. Complete-document materialization and transformation expansion remain possible; a pool bounds active tasks, not bytes.', '',
        '## Validation and commit scope', '',
        'See validation-results.json and validation/*.log.gz for the complete AGENTS.md checklist. Known repository-wide format/Clippy issues and private-parser doctest failures are reported rather than suppressed. Focused I/O tests cover Unicode, combining characters, emoji, NUL/CRLF, empty/large files, invalid UTF-8, missing files, invalid output paths, same-path replacement, partial batch failures, ordering, document isolation and pool reuse. A custom instruction verifies the actual worker executes in the supplied pool.', '',
        'The new commits isolate the safe-read change, dependency removal, pool API and tests from prior uncommitted optimization work. Benchmark source archives include that earlier work equally in before/after; they are the precise measured versions, not a claim that all existing worktree changes belong to these commits.', '',
        '## Reproduce', '',
        '`python3 scripts/benchmark_safe_file_io.py`', '',
        'Inputs are generated deterministically in data/safe-file-io; pipelines are in pipelines/safe-file-io. Source archives, worker/source fingerprints and raw samples are retained beside this report. A new candidate archive is taken on each run, while before-source.tar.gz is preserved.',
    ]
    validation = REPORT / 'validation-results.json'
    if validation.exists():
        history = json.loads(validation.read_text())
        lines += ['', '## Final checklist results', '',
                  '| Scope | Command | Exit |', '|---|---|---:|']
        for phase in ['pre-implementation-commit', 'indexed-implementation', 'pre-benchmark-commit']:
            for result in history.get(phase, []):
                command = ' '.join(result['command'])
                lines.append(f"| {phase} | {command} | {result['exit']} |")
        lines += ['', 'A later validation cache conflict was traced to reusing one target directory for the indexed snapshot and working tree; package artifacts were cleaned and the full checklist was repeated. Its diagnostic log is preserved separately. Benchmark builds each compiled their own library from a fresh workspace before copying the executable; future reproductions use separate per-version target directories.', '', 'The complete test_access suite passes in the final implementation and indexed commit. Other test configurations fail only the two inherited doctests that import a private parser. Format/Clippy failures predate this change; no lint allowances were added. Earlier development logs are preserved; the initial pool-observer fixture was corrected before the final checklist runs.']
    (REPORT / 'report.md').write_text('\n'.join(lines) + '\n')


def main():
    if '--report-only' in sys.argv:
        write_report(json.loads((REPORT / 'results.json').read_text()))
        return
    REPORT.mkdir(parents=True, exist_ok=True)
    data, pipelines, expected = prepare_data()
    with tarfile.open(REPORT / 'after-source.tar.gz', 'w:gz') as archive:
        for name in ['src', 'tests', 'Cargo.toml', 'Cargo.lock']:
            archive.add(ROOT / name, arcname=name)
    worker = ROOT / 'examples/safe_file_io_probe.rs'
    metadata = {
        'rust': subprocess.check_output(['rustc', '--version'], text=True).strip(),
        'bytes_per_file': (data / 'input-0.txt').stat().st_size,
        'input_sha256': digest(data / 'input-0.txt'),
        'worker_sha256': digest(worker),
        'expected_sha256': expected,
        'source_archives_sha256': {version: digest(REPORT / f'{version}-source.tar.gz') for version in ['before', 'after']},
    }
    samples = []
    rng = random.Random(9325)
    with tempfile.TemporaryDirectory(prefix='textforge-safe-io-') as temporary:
        temporary = Path(temporary)
        binaries = {}
        for version in ['before', 'after']:
            snapshot = temporary / version
            snapshot.mkdir()
            with tarfile.open(REPORT / f'{version}-source.tar.gz') as archive:
                archive.extractall(snapshot, filter='data')
            (snapshot / 'examples').mkdir()
            shutil.copyfile(worker, snapshot / 'examples/safe_file_io_probe.rs')
            environment = dict(os.environ, CARGO_TARGET_DIR=str(temporary / f'target-{version}'))
            with (REPORT / f'build-{version}.log').open('w') as log:
                subprocess.run(['cargo', 'build', '--offline', '--locked', '--release', '--example', 'safe_file_io_probe'], cwd=snapshot, env=environment, stdout=log, stderr=log, check=True)
            binaries[version] = temporary / f'probe-{version}'
            shutil.copy2(temporary / f'target-{version}/release/examples/safe_file_io_probe', binaries[version])
        for workload, iterations in [('read-copy', 8), ('mixed-64', 1)]:
            for method, threads in [('file', 1), ('batch', 1), ('batch', 4)]:
                for repetition in range(-1, REPETITIONS):
                    versions = ['before', 'after']
                    rng.shuffle(versions)
                    for version in versions:
                        output = temporary / 'output'
                        output.mkdir(exist_ok=True)
                        timer = temporary / 'time.txt'
                        command = ['/usr/bin/time', '-f', '%e %U %S %M', '-o', str(timer), str(binaries[version]), method, str(pipelines / f'{workload}.tft'), str(data), str(output), str(iterations), str(threads)]
                        subprocess.run(command, check=True)
                        wall, user, system, rss = map(float, timer.read_text().split())
                        for index in range(4):
                            if digest(output / f'output-{index}.txt') != expected[workload]:
                                raise RuntimeError('output mismatch')
                        if repetition >= 0:
                            row = dict(workload=workload, method=method, threads=threads, version=version, repetition=repetition, iterations=iterations, wall_seconds=wall, cpu_seconds=user+system, peak_rss_kib=rss, outputs_verified=True)
                            samples.append(row)
                            print(row, flush=True)
    saved = dict(metadata=metadata, samples=samples)
    (REPORT / 'results.json').write_text(json.dumps(saved, indent=2) + '\n')
    write_report(saved)


if __name__ == '__main__':
    main()
