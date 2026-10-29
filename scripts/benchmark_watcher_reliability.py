#!/usr/bin/env python3
"""Compare the saved pre-fix alternative with the current alternative, same engine.
No compiler/dependency changes: both builds use release + buffered,watchers.
"""
import argparse
import ast
import gzip
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
REPORTS = ROOT / 'docs/benchmarks/watcher-reliability'
DATA = ROOT / 'data/watcher-reliability'
PIPES = ROOT / 'pipelines/watcher-reliability'
NAMES = ['bytes', 'characters', 'words', 'lines', 'byte_delta', 'unchanged', 'next_bytes', 'instruction']
META_NAMES = ['bytes', 'before_bytes', 'next_bytes', 'byte_delta', 'empty', 'instruction', 'prefix', 'suffix']


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def canonical(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()).hexdigest()


def prepare_cases():
    DATA.mkdir(parents=True, exist_ok=True)
    PIPES.mkdir(parents=True, exist_ok=True)
    unit = '2026 INFO user=alice texto=João é 👩🏽‍💻 🇧🇷 banana\n'
    record = unit.encode()
    cases = {}
    for name, mib, steps in [('large-64', 16, 64), ('large-256', 32, 256)]:
        directory = DATA / name
        directory.mkdir(exist_ok=True)
        inp = directory / 'input.txt'
        records = (mib * 1024 ** 2 + len(record) - 1) // len(record)
        with inp.open('wb') as f:
            for _ in range(records // 4096):
                f.write(record * 4096)
            f.write(record * (records % 4096))
        pipe = PIPES / (name + '.tft')
        operations = ['atb "<";\n', 'ate ">";\n', 'raw alice "[USER]";\n', 'tua;\n']
        pipe.write_text(''.join(operations[i % 4] for i in range(steps)))
        prefix, suffix, transformed_unit = '', '', unit
        expected_metrics = []
        previous_bytes = inp.stat().st_size
        for i in range(steps):
            old_state = (prefix, transformed_unit, suffix)
            if i % 4 == 0: prefix = '<' + prefix
            elif i % 4 == 1: suffix += '>'
            elif i % 4 == 2: transformed_unit = transformed_unit.replace('alice', '[USER]')
            else: transformed_unit = transformed_unit.upper()
            size = len(prefix) + len(transformed_unit.encode()) * records + len(suffix)
            expected_metrics.append(dict(bytes=str(size), characters=str(len(prefix)+len(transformed_unit)*records+len(suffix)), words=str(len(transformed_unit.split())*records+bool(suffix)), lines=str(records+bool(suffix)), byte_delta=str(size-previous_bytes), unchanged=str(old_state==(prefix,transformed_unit,suffix)).lower()))
            previous_bytes=size
        for i, metrics in enumerate(expected_metrics):
            metrics['next_bytes'] = expected_metrics[i+1]['bytes'] if i+1<steps else 'none'
        h = hashlib.sha256(prefix.encode())
        record_out = transformed_unit.encode()
        for _ in range(records // 4096): h.update(record_out * 4096)
        h.update(record_out * (records % 4096))
        h.update(suffix.encode())
        cases[name] = dict(mode='process_all_with_watchers', input=str(inp.relative_to(ROOT)), pipeline=str(pipe.relative_to(ROOT)), steps=steps, iterations=1, input_bytes=inp.stat().st_size, input_sha256=digest(inp), expected_metrics=expected_metrics, output_sha256=h.hexdigest(), watchers=NAMES)
    small = DATA / 'collection'
    small.mkdir(exist_ok=True)
    inp = small / 'input.txt'
    inp.write_bytes(record * 16)
    pipe = PIPES / 'collection.tft'
    pipe.write_text('null;\n')
    for name, steps, iterations in [('collection-growth', 20000, 10), ('collection-reuse', 256, 2000)]:
        cases[name] = dict(mode='collect', input=str(inp.relative_to(ROOT)), pipeline=str(pipe.relative_to(ROOT)), steps=steps, iterations=iterations, input_bytes=inp.stat().st_size, input_sha256=digest(inp), output_sha256=digest(inp), watchers=META_NAMES)
    return cases


def verify_report(report, case, label):
    n = case['steps']
    expected_rows = n * 8 if label == 'before' else n
    if len(report) != expected_rows or any(report[n:]):
        raise RuntimeError(f'incorrect report shape: {label}: {len(report)} != {expected_rows}')
    rows = report[:n]
    for i, row in enumerate(rows):
        if set(row) != set(case['watchers']):
            raise RuntimeError('watcher names mismatch')
        if case['mode'] == 'collect':
            expected = dict(bytes=str(case['input_bytes']), before_bytes=str(case['input_bytes']), next_bytes=str(case['input_bytes']), byte_delta='0', empty='false', instruction='null', prefix='true', suffix='true')
        else:
            expected = case['expected_metrics'][i]
        for key, value in expected.items():
            if row[key] != value:
                raise RuntimeError(f'independent watcher mismatch: {i} {key}: {row[key]} != {value}')
    return rows


def build(snapshot, target, worker):
    (snapshot / 'examples').mkdir(exist_ok=True)
    (snapshot / 'examples/comparison_probe.rs').write_bytes(worker.read_bytes())
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    subprocess.run(['cargo','build','--offline','--locked','--release','--features','buffered,watchers','--example','comparison_probe'],cwd=snapshot,env=env,check=True)
    return target / 'release/examples/comparison_probe'


def write_report(saved):
    samples, metadata = saved['samples'], saved['metadata']
    lines = ['# Watcher reliability and scheduling benchmark', '', 'Both versions use the same experimental buffered engine, with buffered,watchers features, locked dependencies and release optimization. Before is the saved working-source snapshot immediately before these fixes; after changes WatcherList collection, scheduling, bounded reuse and JSON publication. No shared-statistics, regex or transform optimization is included.', '', f"Five fresh-process samples per case by default; actual repetitions: {metadata['repetitions']}. One paired warm-up per case is excluded. Version order is randomized within each pair. CPU = user + system; RSS is peak resident process memory, excluding filesystem page cache. GNU time has 10 ms resolution. Builds and validation finish before measurement. Each long-pipeline sample executes once; collection controls are described separately.", '', '| Workload | Version | Wall median (s) | CPU median (s) | Peak RSS median (MiB) | Report bytes | Report rows | Wall range (s) |', '|---|---|---:|---:|---:|---:|---:|---|']
    medians = {}
    for name, case in metadata['cases'].items():
        medians[name] = {}
        for label in ['before','after']:
            group = [row for row in samples if row['case']==name and row['version']==label]
            med = {key: statistics.median(row[key] for row in group) for key in ['wall_seconds','cpu_seconds','peak_rss_kib','report_bytes','report_rows']}
            medians[name][label]=med
            lines.append(f"| {name} | {label} | {med['wall_seconds']:.3f} | {med['cpu_seconds']:.3f} | {med['peak_rss_kib']/1024:.1f} | {med['report_bytes']:.0f} | {med['report_rows']:.0f} | {min(row['wall_seconds'] for row in group):.2f}–{max(row['wall_seconds'] for row in group):.2f} |")
    lines += ['', '## Relative changes (after versus before)', '', '| Workload | Wall | CPU | Peak RSS | Report bytes |', '|---|---:|---:|---:|---:|']
    for name, pair in medians.items():
        changes=[]
        for key in ['wall_seconds','cpu_seconds','peak_rss_kib','report_bytes']:
            a,b=pair['before'][key],pair['after'][key]
            changes.append(f'{100*(b/a-1):+.1f}%' if a else 'below timer resolution')
        lines.append('| '+' | '.join([name]+changes)+' |')
    summary=['## Decision summary','']
    for name in ['large-64','large-256']:
        case=metadata['cases'][name];a,b=medians[name]['before'],medians[name]['after']
        summary += [f"With {case['input_bytes']/1024**2:.0f} MiB and {case['steps']} instructions, wall time is {a['wall_seconds']:.2f} → {b['wall_seconds']:.2f} s, CPU {a['cpu_seconds']:.2f} → {b['cpu_seconds']:.2f} s and peak RSS {a['peak_rss_kib']/1024:.1f} → {b['peak_rss_kib']/1024:.1f} MiB. Each report now has exactly {case['steps']} complete rows rather than {case['steps']*8} rows padded with empty objects.",'']
    summary += ['The fixes improve report integrity and remove avoidable collection allocations. They do not reduce full-document snapshot construction or the repeated scans performed by character/word/line watchers, so large-file end-to-end gains can be much smaller than collection-only gains. Small percentage differences inside the observed timing ranges are inconclusive; no statistical-significance claim is made.', '']
    increases=[]
    for name in ['large-64','large-256']:
        pair=medians[name]
        increases.append(100*(pair['after']['wall_seconds']/pair['before']['wall_seconds']-1))
    growth=medians['collection-growth']; reuse=medians['collection-reuse']
    growth_time=100*(1-growth['after']['wall_seconds']/growth['before']['wall_seconds'])
    growth_memory=100*(1-growth['after']['peak_rss_kib']/growth['before']['peak_rss_kib'])
    reuse_time=100*(1-reuse['after']['wall_seconds']/reuse['before']['wall_seconds'])
    summary += [f"The end-to-end runs show no speedup: median wall time increases {increases[0]:.1f}% for 64 instructions and {increases[1]:.1f}% for 256 instructions. The collection-growth control is {growth_time:.1f}% faster and uses {growth_memory:.1f}% less peak RSS; the reuse control is {reuse_time:.1f}% faster. These collection-only gains must not be presented as large-file pipeline speedups.",'', 'Keep these changes for complete/deterministic reports and lower observer bookkeeping cost. They are not sufficient as a large-file throughput optimization. Further changes to snapshot materialization or repeated metric scans need a separate experiment. The before/after comparison measures the combined package of reliability and scheduling changes, not each change in isolation.','']
    lines[2:2]=summary
    if metadata.get('controls_rerun_for_timer_precision'):
        lines += ['', 'Collection controls were rerun separately with longer fixed iteration counts after all large-file samples finished, to reduce timer quantization. The same source snapshots and worker are verified by hashes before that rerun.']
    lines += ['', '## Workloads and correctness', '', 'large-64: 16 MiB, 64 instructions. large-256: 32 MiB, 256 instructions. Both repeat prefix, suffix, regex replacement and Unicode uppercase, including native and fallback instructions. Growth is bounded: the replacement happens once per original record, and each four-instruction group adds only two delimiter bytes. Eight watchers are identical across versions: bytes, characters, words, lines, byte delta, unchanged, next bytes and instruction. Independent references check every numeric/boolean metric at every step. Full meaningful report rows, including instruction labels, must match across versions. The comparison removes only the known empty legacy trailing rows; the corrected exporter must emit no such rows.', '', 'collection-growth: a fixed ~1 KiB shared context, 20,000 direct watcher steps per run, 10 exports/resets. collection-reuse: the same context, 256 steps per run, 2,000 exports/resets using the same list. These controls isolate scheduling, row allocation and report export; they are not measurements of processing a large file. Eight cheap metadata callbacks avoid full-text scans. Output-file writes and JSON export remain timed.', '', 'All transformed outputs match independent SHA-256 references. Original and new reports are checked after every process; candidate raw JSON bytes must be identical across fresh processes. Sample reports are retained as gzip files, alongside source snapshots and raw samples. The new report keeps the JSON array/string-value schema but removes extra empty rows and sorts keys; callbacks now execute in watcher-name order. Replacement callbacks and reset behavior are covered by regressions.', '', '## Reliability guarantees and limits', '', 'The exporter serializes through a checked buffered writer into a unique same-directory temporary file, flushes it and renames it over the destination only on success. Failed writes preserve the old report and cleanup the temporary file; regression tests inject short writes, write-zero, write/flush failures and concurrent publishers. Publishing is atomic on the tested Linux filesystem. This does not promise persistence across power loss, preserve old inode permissions, or recover from process abort/OOM. Reuse retains at most 256 empty row maps with capacity at most 128 entries; values and oversized storage are released. No new dependency or public signature was added.', '', '## Reproduction', '', '`python3 scripts/benchmark_watcher_reliability.py`', '', 'Requires Linux, GNU time, Python 3.12+, Rust and cached locked Cargo dependencies. Data: data/watcher-reliability/. Pipelines: pipelines/watcher-reliability/. Before source: baseline-source.tar.gz. After source: candidate-source.tar.gz. Raw results: results.json. `--report-only` regenerates this report without rerunning measurements. Validation: validation-results.json and validation/*.log.gz.']
    validation=REPORTS/'validation-results.json'
    if validation.exists():
        lines += ['', '## Validation', '', '| Command | Exit |', '|---|---:|']
        for check in json.loads(validation.read_text()):
            lines.append('| '+' '.join(check['command'])+' | '+str(check['exit'])+' |')
        lines += ['', 'Full test_access passes, including all reliability regressions and doctests. The standalone watchers configuration retains the two known private-parser doctest failures; its unit/integration tests pass. Repository formatting and strict Clippy retain inherited diagnostics. No new Clippy diagnostics target the optimized watcher module or tests.']
    (REPORTS/'report.md').write_text('\n'.join(lines)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repetitions',type=int,default=5)
    parser.add_argument('--report-only',action='store_true')
    parser.add_argument('--controls-only',action='store_true',help='rerun longer collection controls, retaining previously measured large cases')
    args=parser.parse_args()
    if args.report_only:
        write_report(json.loads((REPORTS/'results.json').read_text()));return
    if args.repetitions<3: parser.error('use at least three repetitions')
    REPORTS.mkdir(parents=True,exist_ok=True)
    cases=prepare_cases()
    worker=ROOT/'examples/comparison_probe.rs'
    if args.controls_only:
        previous=json.loads((REPORTS/'results.json').read_text())
        metadata=previous['metadata']
        if args.repetitions!=metadata['repetitions']:
            raise RuntimeError('controls-only requires the original repetition count')
        for name, expected in metadata['archives'].items():
            if digest(REPORTS/name)!=expected:
                raise RuntimeError('saved source archive changed')
        current_sources={str(path.relative_to(ROOT)):digest(path) for path in sorted((ROOT/'src').rglob('*.rs'))}
        if current_sources!=metadata['candidate_sources'] or digest(worker)!=metadata['worker_sha256']:
            raise RuntimeError('source changed since large-file measurements; rerun the full benchmark')
        cases={name:case for name,case in cases.items() if case['mode']=='collect'}
        metadata['cases'].update(cases)
        metadata['controls_rerun_for_timer_precision']=True
        samples=[row for row in previous['samples'] if row['case'] not in cases]
    else:
        with tarfile.open(REPORTS/'candidate-source.tar.gz','w:gz') as archive:
            for name in ['src','tests','Cargo.toml','Cargo.lock']:
                archive.add(ROOT/name,arcname=name)
        metadata=dict(repetitions=args.repetitions,cases=cases,rustc=subprocess.check_output(['rustc','-Vv'],text=True),platform=platform.platform(),cpu=Path('/proc/cpuinfo').read_text().split('model name')[1].split('\n')[0].strip(' :\t'),features=['buffered','watchers'],worker_sha256=digest(worker),archives={name:digest(REPORTS/name) for name in ['baseline-source.tar.gz','candidate-source.tar.gz']},candidate_sources={str(path.relative_to(ROOT)):digest(path) for path in sorted((ROOT/'src').rglob('*.rs'))})
        samples=[]
    references={};raw_hashes={};rng=random.Random(20261028)
    with tempfile.TemporaryDirectory(prefix='textforge-reliability-') as temporary:
        temporary=Path(temporary);binaries={}
        for label,archive in [('before','baseline-source.tar.gz'),('after','candidate-source.tar.gz')]:
            snapshot=temporary/label;snapshot.mkdir()
            with tarfile.open(REPORTS/archive) as source:
                source.extractall(snapshot,filter='data')
            binaries[label]=build(snapshot,temporary/('target-'+label),worker)
        env=dict(os.environ,NO_COLOR='1',RAYON_NUM_THREADS='4')
        jobs=list(cases)
        for repetition in range(-1,args.repetitions):
            rng.shuffle(jobs)
            for name in jobs:
                labels=['before','after'];rng.shuffle(labels)
                for label in labels:
                    case=cases[name];output=temporary/'output';output.mkdir(exist_ok=True)
                    for stale in output.iterdir(): stale.unlink()
                    metrics=temporary/'time.txt'
                    count=case['steps'] if case['mode']=='collect' else 1
                    command=['/usr/bin/time','-f','%e %U %S %M','-o',str(metrics),str(binaries[label]),case['mode'],str(ROOT/case['pipeline']),str(ROOT/case['input']),str(output),str(case['iterations']),str(count)]
                    subprocess.run(command,env=env,stdout=subprocess.DEVNULL,check=True,timeout=240)
                    wall,user,system,rss=map(float,metrics.read_text().split())
                    if digest(output/'result-0.txt')!=case['output_sha256']: raise RuntimeError('output digest mismatch')
                    report_path=output/'watchers.json';report=json.loads(report_path.read_text());meaningful=verify_report(report,case,label)
                    fingerprint=canonical(meaningful)
                    if name in references and references[name]!=fingerprint: raise RuntimeError('before/after report mismatch')
                    references[name]=fingerprint
                    raw=digest(report_path)
                    if label=='after':
                        if name in raw_hashes and raw_hashes[name]!=raw: raise RuntimeError('non-deterministic candidate JSON')
                        raw_hashes[name]=raw
                    with gzip.open(REPORTS/(name+'-'+label+'.json.gz'),'wb') as saved_report:
                        saved_report.write(report_path.read_bytes())
                    row=dict(case=name,version=label,repetition=repetition,wall_seconds=wall,cpu_seconds=user+system,user_seconds=user,system_seconds=system,peak_rss_kib=int(rss),report_bytes=report_path.stat().st_size,report_rows=len(report),meaningful_rows=len(meaningful),report_sha256=raw,canonical_metrics_sha256=fingerprint,output_sha256_match=True)
                    if repetition>=0:
                        samples.append(row)
                        (REPORTS/'results.json').write_text(json.dumps(dict(metadata=metadata,samples=samples),indent=2,ensure_ascii=False))
                    print(f'{repetition=} {name} {label}: wall={wall:.2f}s cpu={user+system:.2f}s rss={rss/1024:.1f}MiB rows={len(report)} bytes={report_path.stat().st_size}',flush=True)
    write_report(dict(metadata=metadata,samples=samples))

if __name__=='__main__': main()
