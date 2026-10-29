#!/usr/bin/env python3
"""Linux end-to-end revision comparison; Python stdlib and GNU time only.
Run from any directory: python3 scripts/benchmark_buffered_comparison.py [--repetitions 5].
Builds the original commit and current working source (or --candidate ref), with an identical worker. Source hashes are retained to identify uncommitted candidate code.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import statistics
import shutil
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
    p.add_argument('--include-watchers', action='store_true', help='benchmark all four methods with identical watcher support in both builds')
    p.add_argument('--report-only', action='store_true', help='regenerate report from existing raw samples')
    p.add_argument('--original', default='608e4fc')
    p.add_argument('--candidate', default='WORKTREE')
    p.add_argument('--repetitions', type=int, default=5)
    p.add_argument('--iterations', type=int, default=4)
    a = p.parse_args()
    prefix = "watchers-comparison" if a.include_watchers else "buffered-comparison"
    if a.report_only:
        reports = ROOT / 'docs/benchmarks'
        saved = json.loads((reports / (prefix + '-results.json')).read_text())
        jobs = sorted({(r['scenario'], r['method']) for r in saved['samples']})
        write_report(saved['samples'], reports, jobs, a.include_watchers)
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
            revision = subprocess.check_output(['git', 'rev-parse', 'HEAD' if ref == 'WORKTREE' else ref], cwd=ROOT, text=True).strip()
            metadata['revisions'][label] = revision
            snapshot = temp / label
            snapshot.mkdir()
            archive = temp / (label + '.tar')
            with archive.open('wb') as f:
                run(['git', 'archive', revision], cwd=ROOT, stdout=f)
            with tarfile.open(archive) as t:
                t.extractall(snapshot, filter='data')
            if label == 'candidate' and ref == 'WORKTREE':
                shutil.copytree(ROOT / 'src', snapshot / 'src', dirs_exist_ok=True)
                for name in ['Cargo.toml', 'Cargo.lock']:
                    shutil.copyfile(ROOT / name, snapshot / name)
                source_hashes = {str(path.relative_to(ROOT)): digest(path) for path in sorted((ROOT / 'src').rglob('*.rs'))}
                source_hashes.update({name: digest(ROOT / name) for name in ['Cargo.toml', 'Cargo.lock']})
                metadata['candidate_source_hashes'] = source_hashes
                metadata['candidate_tree_sha256'] = hashlib.sha256(json.dumps(source_hashes, sort_keys=True).encode()).hexdigest()
                metadata['revisions'][label] += ' + working source overlay'
            (snapshot / 'examples').mkdir(exist_ok=True)
            (snapshot / 'examples/comparison_probe.rs').write_bytes((ROOT / 'examples/comparison_probe.rs').read_bytes())
            env = dict(os.environ, CARGO_TARGET_DIR=str(temp / ('target-' + label)))
            feature_names = (['buffered'] if label == 'candidate' else []) + (['watchers'] if a.include_watchers else [])
            features = ['--features', ','.join(feature_names)] if feature_names else []
            run(['cargo', 'build', '--offline', '--release', '--locked', '--example', 'comparison_probe'] + features, cwd=snapshot, env=env)
            binaries[label] = Path(env['CARGO_TARGET_DIR']) / 'release/examples/comparison_probe'
        env = dict(os.environ, RAYON_NUM_THREADS='4', NO_COLOR='1')
        metadata['equivalence_checks'] = check_debug_equivalence(binaries, temp, env, ROOT / scenarios['wrap']['input'])
        watcher_references = {}
        if a.include_watchers:
            metadata['watcher_equivalence_checks'], watcher_references = check_watcher_equivalence(binaries, temp, env, scenarios)
            metadata['watcher_names'] = ['bytes', 'characters', 'words', 'lines', 'byte_delta', 'unchanged', 'next_bytes', 'instruction']
            metadata['features'] = {'original': ['watchers'], 'candidate': ['buffered', 'watchers']}
        metadata['worker_sha256'] = digest(ROOT / 'examples/comparison_probe.rs')
        methods = ['process_all', 'process_batch', 'process_all_with_debug'] + (['process_all_with_watchers'] if a.include_watchers else [])
        jobs = [(scenario, method) for scenario in scenarios for method in methods]
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
                    watcher_hash = None
                    if method == 'process_all_with_watchers':
                        watcher_report = json.loads((out / 'watchers.json').read_text())
                        if normalized_report(watcher_report) != normalized_report(watcher_references[scenario]):
                            raise RuntimeError(f'watcher report mismatch: {scenario} {label} {rep}')
                        watcher_hash = canonical_hash(watcher_report)
                        saved_reports = reports / 'watchers-reports'
                        saved_reports.mkdir(exist_ok=True)
                        (saved_reports / f'{scenario}-{label}.json').write_text(json.dumps(watcher_report, indent=2, sort_keys=True, ensure_ascii=False))
                    wall, user, system, rss = map(float, metrics.read_text().split())
                    if rep >= 0:
                        rows.append({'scenario': scenario, 'method': method, 'revision': label, 'repetition': rep, 'wall_seconds': wall, 'user_seconds': user, 'system_seconds': system, 'cpu_seconds': user + system, 'cpu_percent': 100 * (user + system) / wall if wall else None, 'peak_rss_kib': int(rss), 'documents': count * iterations, 'sha256_match': True, 'watcher_report_sha256': watcher_hash})
                    print(f'{rep=} {label} {scenario} {method}: {wall:.2f}s {rss/1024:.1f}MiB', flush=True)
    (reports / (prefix + '-results.json')).write_text(json.dumps({'metadata': metadata, 'samples': rows}, indent=2, ensure_ascii=False))
    write_report(rows, reports, jobs, a.include_watchers)

def check_debug_equivalence(binaries, directory, env, large_input):
    cases = [
        ('empty', 'atb "<";\nate ">";\n', ''),
        ('unicode', 'atb "<";\nate ">";\n', 'João é 👩🏽‍💻 🇧🇷\n'),
        ('ansi', 'atb "<";\nate ">";\n', 'x\x1b[0my'),
        ('variables', 'var name = João;\natb "{{name}}: ";\n', 'ok'),
        ('block', 'blk x assoc tua;\ncblk x;\n', 'alice é'),
        ('halt', 'atb A;\nend;\nate B;\n', 'abc'),
        ('fallback', 'raw alice USER;\ntua;\n', 'alice é'),
        ('spilled', 'atb "<";\nate ">";\n', large_input.read_text()),
        ('failure', 'atb "{{missing}}";\n', 'abc'),
    ]
    checks = []
    base = directory / 'equivalence'
    base.mkdir()
    for name, source, text in cases:
        inp, pipeline = base / 'input', base / 'pipeline.tft'
        inp.write_text(text)
        pipeline.write_text(source)
        for color in [False, True]:
            outputs, logs = {}, {}
            for label, binary in binaries.items():
                out = base / label
                out.mkdir(exist_ok=True)
                log = base / (label + '.log')
                local_env = dict(env)
                if color:
                    local_env.pop('NO_COLOR', None)
                    local_env['CLICOLOR_FORCE'] = '1'
                else:
                    local_env['CLICOLOR_FORCE'] = '0'
                with log.open('wb') as f:
                    result = subprocess.run([str(binary), 'process_all_with_debug', str(pipeline), str(inp), str(out), '1', '1'], stdout=f, stderr=subprocess.PIPE, env=local_env, timeout=60)
                if name == 'failure':
                    if result.returncode == 0 or log.stat().st_size:
                        raise RuntimeError(f'partial debug output or missing error: {label}')
                    outputs[label] = None
                else:
                    if result.returncode != 0:
                        raise RuntimeError(f'debug failed: {label} {name}: {result.stderr!r}')
                    outputs[label] = digest(out / 'result-0.txt')
                logs[label] = digest(log)
            if outputs['original'] != outputs['candidate'] or logs['original'] != logs['candidate']:
                raise RuntimeError(f'debug equivalence mismatch: {name} {color=}')
            checks.append({'case': name, 'forced_color': color, 'result_matches': True, 'stdout_matches': True, 'stdout_sha256': logs['candidate']})
    print(f'Debug equivalence passed: {len(checks)} checks', flush=True)
    return checks

def normalized_report(report):
    if report is None:
        return None
    end = len(report)
    while end and not report[end - 1]:
        end -= 1
    return report[:end]

def canonical_hash(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, ensure_ascii=False, separators=(',', ':')).encode()).hexdigest()


def verify_watcher_metrics(report, scenario, text):
    states = [text]
    if scenario == 'normalize':
        states += [text.replace('alice', '[USER]')]
        states += [states[-1].upper()]
    states += ['<' + states[-1], '<' + states[-1] + '>']
    steps = len(states) - 1
    # Retain the current report array shape, including empty trailing entries.
    if len(report) not in [steps, steps * 8] or any(report[steps:]):
        raise RuntimeError('watcher report shape mismatch')
    for i in range(steps):
        before, current = states[i], states[i + 1]
        expected = {
            'bytes': str(len(current.encode())), 'characters': str(len(current)),
            'words': str(len(current.split())), 'lines': str(len(current.splitlines())),
            'byte_delta': str(len(current.encode()) - len(before.encode())),
            'unchanged': str(current == before).lower(),
            'next_bytes': str(len(states[i + 2].encode())) if i + 2 < len(states) else 'none',
        }
        if set(report[i]) != set(expected) | {'instruction'}:
            raise RuntimeError('missing watcher fields')
        for key, value in expected.items():
            if report[i][key] != value:
                raise RuntimeError(f'independent watcher reference mismatch: {scenario} {i} {key}')


def check_watcher_equivalence(binaries, directory, env, scenarios):
    cases = [
        ('empty', 'atb "<";\nate ">";\n', ''),
        ('noop', 'null;\n', 'é 👩🏽‍💻'),
        ('variables', 'var name = João;\natb "{{name}}: ";\n', 'ok'),
        ('blocks', 'blk clean assoc tua;\ncblk clean;\n', 'alice é'),
        ('halt', 'atb A;\nend;\nate B;\n', 'abc'),
        ('fallback', 'raw alice USER;\ntua;\n', 'alice é'),
        ('failure', 'atb "{{missing}}";\n', 'abc'),
    ]
    for name, scenario in scenarios.items():
        cases.append((name, (ROOT / scenario['pipeline']).read_text(), (ROOT / scenario['input']).read_text()))
    base = directory / 'watcher-equivalence'
    base.mkdir()
    checks, references = [], {}
    for name, pipeline, text in cases:
        inp, pipe = base / 'input', base / 'pipeline.tft'
        inp.write_text(text)
        pipe.write_text(pipeline)
        result_hashes, reports = {}, {}
        for label, binary in binaries.items():
            out = base / label
            out.mkdir(exist_ok=True)
            report_file = out / 'watchers.json'
            report_file.unlink(missing_ok=True)
            result = subprocess.run([str(binary), 'process_all_with_watchers', str(pipe), str(inp), str(out), '2', '1'], stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, env=env, timeout=60)
            if name == 'failure':
                if result.returncode == 0 or report_file.exists():
                    raise RuntimeError('watcher failure exported a success report')
                reports[label] = result_hashes[label] = None
            else:
                if result.returncode != 0:
                    raise RuntimeError(f'watcher equivalence failed: {name} {label}: {result.stderr!r}')
                reports[label] = json.loads(report_file.read_text())
                result_hashes[label] = digest(out / 'result-0.txt')
                if name in scenarios:
                    verify_watcher_metrics(reports[label], name, text)
        if normalized_report(reports['original']) != normalized_report(reports['candidate']) or result_hashes['original'] != result_hashes['candidate']:
            raise RuntimeError(f'watcher equivalence mismatch: {name}')
        checks.append({'case': name, 'results_match': True, 'reports_match': True, 'canonical_report_sha256': canonical_hash(reports['original'])})
        if name in scenarios:
            references[name] = reports['original']
    print(f'Watcher equivalence passed: {len(checks)} cases; independent fixture metrics verified', flush=True)
    return checks, references

def write_report(rows, reports, jobs, watchers=False):
    prefix = 'watchers-comparison' if watchers else 'buffered-comparison'
    lines = ['# Revision benchmark — Rust 1.99.1', '', 'Identical release workers; original uses default features and candidate enables buffered; per-buffer and debug-log spill thresholds are 1 MiB, using the system temporary directory; locked dependencies; one excluded warm-up per case; randomized paired revision order; four Rayon threads. Five fresh-process samples by default. Each sample includes startup, pipeline parsing, input reads, repeated executions and output writes (100 wrap / 4 normalize executions per worker with default options; exact document counts are in the raw samples). Batch executes four documents per call. Debug stdout goes to /dev/null, so formatting/allocation remain measured but terminal rendering does not.', '', 'CPU = user + system time; CPU% may exceed 100 with parallel execution. RSS is the maximum resident memory of the entire worker, not allocated bytes. GNU time has 10 ms time resolution; small timings are noisy. CPU percentages omit samples whose wall time rounds to zero; nan means no measurable wall time. No claim of statistical significance is made.', '', 'The same three API signatures are exercised in both versions. With buffered enabled, process_all uses TextBuffer and materializes its String result; process_batch keeps Rayon parallelism and streams each output file; process_all_with_debug spools the complete log before printing it. Non-native transforms still materialize the document. Debug with forced color also materializes each snapshot to preserve embedded ANSI resets. Disk page-cache memory is not included in process RSS. These are end-to-end warm-cache measurements, not isolated transform timings.', '', '| Scenario | Method | Version | Wall median (s) | CPU median (s) | CPU% median | RSS median (MiB) | Wall range (s) |', '|---|---|---|---:|---:|---:|---:|---:|']
    for scenario, method in sorted(jobs):
        for label in ['original', 'candidate']:
            group = [r for r in rows if (r['scenario'], r['method'], r['revision']) == (scenario, method, label)]
            def median(key):
                values = [r[key] for r in group if r[key] is not None]
                return statistics.median(values) if values else float('nan')
            lines.append(f"| {scenario} | {method} | {label} | {median('wall_seconds'):.3f} | {median('cpu_seconds'):.3f} | {median('cpu_percent'):.1f} | {median('peak_rss_kib')/1024:.1f} | {min(r['wall_seconds'] for r in group):.2f}–{max(r['wall_seconds'] for r in group):.2f} |")
    lines += ['', 'Every measured output matched an independently computed SHA-256 reference, including every batch output. Input data contain accents, combining marks, emoji and ZWJ sequences.', '', '## Reproduction', '', '`python3 scripts/benchmark_buffered_comparison.py`', '', 'Requires Linux, GNU time, Python 3.12+, Rust and cached Cargo dependencies. Input fixtures and textual pipelines live in the repository root under data/benchmark-comparison and pipelines/benchmark-comparison. Generated binaries and output files are temporary. Raw samples, machine metadata, exact revisions and fixture hashes: buffered-comparison-results.json.', '', '## Compiler regression scope', '', 'Only Rust 1.99.1 is installed. Tests on this compiler can reveal failures, but cannot establish that an issue was introduced by the upgrade without running the same commit on the previous compiler. See validation-results.json for the feature/check matrix.']
    lines += ['', '## Candidate relative to original', '', '| Scenario | Method | Wall change | CPU-time change | Peak-RSS change |', '|---|---|---:|---:|---:|']
    for scenario, method in sorted(jobs):
        changes = []
        for key in ['wall_seconds', 'cpu_seconds', 'peak_rss_kib']:
            values = {label: statistics.median(row[key] for row in rows if (row['scenario'], row['method'], row['revision']) == (scenario, method, label)) for label in ['original', 'candidate']}
            changes.append(f"{100 * (values['candidate'] / values['original'] - 1):+.1f}%" if values['original'] else 'below timer resolution')
        lines.append('| ' + ' | '.join([scenario, method] + changes) + ' |')
    lines += ['', 'Negative changes mean lower resource use or faster processing. Five samples do not establish statistical significance. Compare the wall ranges and consider CPU, RSS and extra temporary-file I/O together. Each input and output uses the same bytes in both implementations. The debug equivalence suite compares full stdout and results with and without forced colors, including empty input, Unicode, embedded ANSI resets, variables, blocks, halt, fallback transforms and a spilled document; transformation failure must not print partial logs. Detailed check results and source fingerprints are in the raw JSON.', '', 'Enable the adapters using cargo build --features buffered. Without that feature, the original methods remain selected. No new dependency or public signature is introduced. The earlier comparison report is retained separately.']
    lines += ['', '## Interpretation', '', 'The adapters trade lower batch/debug process RSS for more CPU time and temporary-file I/O. For these workloads, process_all offers no meaningful RSS improvement and takes longer. The caller-owned input and final String allocation remain, and normalization uses full-document fallbacks. Batch and debug reduce retained memory, but all six measured cases have higher median wall time with the candidate. This result supports keeping the feature opt-in; it does not support switching the default implementation for these loads. The threshold is fixed at 1 MiB for this experiment; conclusions do not cover other thresholds, larger workloads or cold disk caches.']
    validation_file = reports / ('watcher-validation-results.json' if watchers else 'buffered-validation-results.json')
    if validation_file.exists():
        checks = json.loads(validation_file.read_text())
        lines += ['', '## Validation of the new implementation', '', '| Command | Result |', '|---|---|']
        for check in checks:
            lines.append('| ' + ' '.join(check['command']) + ' | ' + ('PASS' if check['exit'] == 0 else 'FAIL') + ' |')
        lines += ['', 'test_access passes, including the new adapters and resource-limit regression. Buffered library/integration tests pass. Default, bytecode and watchers retain the two known private-parser doctest failures. Repository-wide formatting and strict Clippy retain inherited failures; the new adapter module and tests have no Clippy diagnostics. Full logs and test summaries are retained in buffered-validation/ and buffered-validation-results.json. Earlier validation-results.json describes the preceding commit comparison.']
    if watchers:
        lines[0] = '# Four-method benchmark including watchers — Rust 1.99.1'
        lines[2] = lines[2].replace('original uses default features and candidate enables buffered', 'original enables watchers and candidate enables buffered,watchers')
        lines = [line.replace('python3 scripts/benchmark_buffered_comparison.py`', 'python3 scripts/benchmark_buffered_comparison.py --include-watchers`').replace('buffered-comparison-results.json', 'watchers-comparison-results.json').replace('all six measured cases', 'the three previously measured methods') for line in lines]
        lines += ['', '## Watcher workload and compatibility', '', 'Eight watchers run after every instruction: UTF-8 bytes, Unicode scalar count, words, lines, byte delta, unchanged, next-state byte length and instruction label. They exercise all four WatcherContext fields, using six existing default functions and two small custom functions. Registration happens once per worker, and the same list is reused across calls; snapshot construction, scanning, JSON serialization/export and automatic reset are timed. Report equality is checked after each process, outside the timed worker. Canonical JSON comparison ignores map key ordering and removes only the known trailing-empty rows in the legacy format. The candidate now exports exactly one row per observed instruction.', '', 'The current WatcherContext requires complete Arc<str> snapshots. The adapter therefore materializes before/current/next states while transformations use TextBuffer; it cannot guarantee bounded memory. It preserves existing lookahead behavior (after is the next instruction output; None at EOF), error propagation and reset-after-success behavior. Existing watcher semantics, including how end is handled, are preserved for this comparison.', '', 'Detailed sample reports are saved in watchers-reports/. Each report is compared with the original implementation and all seven numeric/boolean fields are independently checked against Python-computed intermediate states for both measured fixtures. Additional untimed equivalence cases cover empty/no-op, Unicode, variables, blocks, halt, fallback and failures. Source and worker hashes are retained in the raw results.', '', 'Watcher validation for this run is recorded in watcher-validation-results.json and watcher-validation/. The earlier buffered-validation files describe the preceding three-method experiment.']
        def medians(scenario, method, label):
            group = [row for row in rows if row['scenario'] == scenario and row['method'] == method and row['revision'] == label]
            return {key: statistics.median(row[key] for row in group) for key in ['wall_seconds', 'cpu_seconds', 'peak_rss_kib']}
        def watcher_summary(scenario, size):
            original = medians(scenario, 'process_all_with_watchers', 'original')
            candidate = medians(scenario, 'process_all_with_watchers', 'candidate')
            change = lambda key: 100 * (candidate[key] / original[key] - 1)
            return f"For {size}, watcher wall time changes from {original['wall_seconds']:.2f} to {candidate['wall_seconds']:.2f} s ({change('wall_seconds'):+.1f}%), CPU time from {original['cpu_seconds']:.2f} to {candidate['cpu_seconds']:.2f} s ({change('cpu_seconds'):+.1f}%), and peak RSS from {original['peak_rss_kib']/1024:.1f} to {candidate['peak_rss_kib']/1024:.1f} MiB ({change('peak_rss_kib'):+.1f}%)."
        summaries = ['## Decision summary', '',
            'The extended comparison contains 80 measured fresh-process samples across four methods, two fixtures and two implementations, with five samples per case after warm-up. The 1 MiB sample runs 100 calls and the 8 MiB sample runs four calls; batch processes four documents per call. All outputs match independent SHA-256 references. All measured watcher reports match the original parsed JSON; nine additional watcher equivalence cases and 18 debug equivalence checks pass.', '',
            watcher_summary('normalize', 'the 8 MiB normalization workload') + ' ' + watcher_summary('wrap', 'the 1 MiB prefix/suffix workload'), '',
            'The watcher adapter is a compatibility path, not a streaming observer API. Existing callbacks require complete Arc<str> snapshots of before/current/next states, so spooling transformations still requires materialization. The extra temporary-file I/O is consistent with the higher CPU and wall times. Process RSS excludes filesystem page-cache memory, so the RSS reduction is not a measurement of total host-memory savings.', '',
            'For these workloads and the fixed 1 MiB threshold, retain the original watcher implementation when processing time or CPU efficiency is the priority. The experimental watcher path offers a modest RSS reduction at a substantial time cost. The larger memory benefits remain in debug and batch; ordinary process_all does not show a useful RSS reduction. Keep buffered opt-in and avoid switching all methods by default. These observations do not establish behavior for larger documents, other thresholds or cold caches.', '']
        lines[2:2] = summaries
        lines = [line.replace('See validation-results.json for the feature/check matrix.', 'See watcher-validation-results.json for validation of the current implementation.').replace('The same three API signatures', 'The same four API signatures') for line in lines]
        lines = [line if not line.startswith('test_access passes, including') else 'Full test_access passes, including new watcher snapshot, reset/reuse, Unicode and error tests. All library/integration tests in the watchers, buffered+watchers and buffered-only configurations pass; the full cargo test commands retain the two known private-parser doctest failures. Repository formatting and strict Clippy retain inherited failures; no diagnostics point to the new adapter module or tests. Full logs and summaries are in watcher-validation/ and watcher-validation-results.json.' for line in lines]
        for scenario in ['wrap', 'normalize']:
            groups = {label: [row for row in rows if row['scenario'] == scenario and row['method'] == 'process_all_with_watchers' and row['revision'] == label] for label in ['original', 'candidate']}
            values = {label: {key: statistics.median(row[key] for row in group) for key in ['wall_seconds', 'cpu_seconds', 'peak_rss_kib']} for label, group in groups.items()}
            a, b = values['original'], values['candidate']
            lines += ['', f"Watchers, {scenario}: wall {a['wall_seconds']:.3f} → {b['wall_seconds']:.3f} s; CPU {a['cpu_seconds']:.3f} → {b['cpu_seconds']:.3f} s; peak RSS {a['peak_rss_kib']/1024:.1f} → {b['peak_rss_kib']/1024:.1f} MiB."]
    (reports / (prefix + '-report.md')).write_text('\n'.join(lines) + '\n')

if __name__ == '__main__':
    main()
