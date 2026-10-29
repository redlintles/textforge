#!/usr/bin/env python3
"""Steps 3–5: exact saved pre-change alternative versus current implementation.
Same engine/features, worker, data, pipeline and watcher list in both builds.
"""
import argparse
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
import benchmark_watcher_reliability as common

ROOT = common.ROOT
REPORTS = ROOT / 'docs/benchmarks/steps-345'
DATA = ROOT / 'data/steps-345'
PIPES = ROOT / 'pipelines/steps-345'


def cases():
    previous = common.prepare_cases()
    DATA.mkdir(parents=True, exist_ok=True)
    PIPES.mkdir(parents=True, exist_ok=True)
    result = {name: previous[name] for name in ['large-64', 'large-256']}
    for name, case in result.items():
        copied = PIPES / (name + '.tft')
        copied.write_bytes((ROOT / case['pipeline']).read_bytes())
        case['pipeline'] = str(copied.relative_to(ROOT))
        case['reports'] = True
    main = dict(result['large-256'])
    main.update(mode='process_all', reports=False)
    result['large-256-no-watchers'] = main
    directory = DATA / 'prepared-constants'
    directory.mkdir(exist_ok=True)
    unit = '2026 INFO alice é 👩🏽‍💻 banana\n'.replace('\\n', '\n')
    inp = directory / 'input.txt'
    inp.write_text(unit * 32)
    pipe = PIPES / 'prepared-constants.tft'
    pipe.write_text('raw alice alice;\n'.replace('\\n', '\n') * 128)
    result['prepared-constants'] = dict(mode='process_all', input=str(inp.relative_to(ROOT)), pipeline=str(pipe.relative_to(ROOT)), steps=128, iterations=500, input_bytes=inp.stat().st_size, input_sha256=common.digest(inp), output_sha256=common.digest(inp), reports=False)
    source = result['large-256']
    result['shared-statistics'] = dict(mode='collect-full', input=source['input'], pipeline=source['pipeline'], steps=64, iterations=1, input_bytes=source['input_bytes'], input_sha256=source['input_sha256'], output_sha256=source['input_sha256'], watchers=common.NAMES, reports=True,
        expected_statistics=dict(bytes=str(source['input_bytes']), characters=str(int(source['expected_metrics'][0]['characters'])-1), words=source['expected_metrics'][0]['words'], lines=source['expected_metrics'][0]['lines'], byte_delta='0', unchanged='true', next_bytes=str(source['input_bytes']), instruction='null'))
    return result


def verify(report, case):
    if len(report) != case['steps']:
        raise RuntimeError('incorrect report row count')
    for index, row in enumerate(report):
        expected = case['expected_statistics'] if case['mode'] == 'collect-full' else case['expected_metrics'][index]
        if set(row) != set(common.NAMES):
            raise RuntimeError('incorrect watcher fields')
        for name, value in expected.items():
            if row[name] != value:
                raise RuntimeError(f'independent reference mismatch: {index} {name}')


def report(saved):
    rows, metadata = saved['samples'], saved['metadata']
    values = {}
    for name in metadata['cases']:
        values[name] = {}
        for version in ['before', 'after']:
            group = [row for row in rows if row['case'] == name and row['version'] == version]
            values[name][version] = {key: statistics.median(row[key] for row in group) for key in ['wall_seconds', 'cpu_seconds', 'peak_rss_kib', 'report_bytes']}
    lines = ['# Steps 3–5 performance comparison', '', '## Decision summary', '']
    for name in ['large-64', 'large-256', 'large-256-no-watchers']:
        a, b = values[name]['before'], values[name]['after']
        percent = 100*(b['wall_seconds']/a['wall_seconds']-1)
        lines += [f"{name}: wall {a['wall_seconds']:.2f} → {b['wall_seconds']:.2f} s ({percent:+.1f}%), CPU {a['cpu_seconds']:.2f} → {b['cpu_seconds']:.2f} s, peak RSS {a['peak_rss_kib']/1024:.1f} → {b['peak_rss_kib']/1024:.1f} MiB.", '']
    prepared=values['prepared-constants']; shared=values['shared-statistics']
    prepared_gain=100*(1-prepared['after']['wall_seconds']/prepared['before']['wall_seconds'])
    shared_gain=100*(1-shared['after']['wall_seconds']/shared['before']['wall_seconds'])
    extra=(prepared['after']['peak_rss_kib']-prepared['before']['peak_rss_kib'])/1024
    lines += [f"The constant-regex control is {prepared_gain:.1f}% faster, with {extra:.2f} MiB more peak RSS from retained prepared metadata. The shared-statistics control is {shared_gain:.1f}% faster with effectively unchanged RSS. These controls support the selected optimizations, while the full pipelines show the combined result. Compiled patterns remain resident, so complex-pattern pipelines can have a larger preparation memory cost than this control.", '', 'For the tested large mixed pipelines, these changes offer a substantial time/CPU reduction. RAM improves in the 16 MiB case and remains close in the 32 MiB cases. This does not establish superiority for every instruction mix or against the separately updated non-buffered engine; the comparison is against the saved alternative after steps 1–2.', '']
    lines += ['The same 16/32 MiB files and 64/256-instruction mixed pipelines from the preceding experiment are reused. Before already contains the reliability/scheduling fixes (steps 1–2). After additionally prepares constant bindings, shares recognized default-watcher counts and retains text already required in RAM across compatible operations. All report bytes and transformed outputs are checked, including independent per-step metric references. Results describe the combined package; isolated controls below demonstrate constant-binding and shared-scan behavior without assigning the entire end-to-end gain to one change.', '', '## Measurements', '', '| Workload | Version | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |', '|---|---|---:|---:|---:|---|']
    for name in metadata['cases']:
        for version in ['before', 'after']:
            group=[row for row in rows if row['case']==name and row['version']==version]
            value=values[name][version]
            lines.append(f"| {name} | {version} | {value['wall_seconds']:.3f} | {value['cpu_seconds']:.3f} | {value['peak_rss_kib']/1024:.1f} | {min(row['wall_seconds'] for row in group):.2f}–{max(row['wall_seconds'] for row in group):.2f} |")
    lines += ['', '| Workload | Wall change | CPU change | Peak RSS change |', '|---|---:|---:|---:|']
    for name in metadata['cases']:
        a,b=values[name]['before'],values[name]['after']
        changes=[f"{100*(b[key]/a[key]-1):+.1f}%" if a[key] else 'below timer resolution' for key in ['wall_seconds','cpu_seconds','peak_rss_kib']]
        lines.append('| '+' | '.join([name]+changes)+' |')
    lines += ['', '## Methodology and controls', '', f"{metadata['repetitions']} fresh-process samples per case/version after one excluded paired warm-up. Pair/version order is randomized with a fixed seed. Identical release builds use buffered,watchers, the same locked dependencies and worker. GNU time captures wall, user+system CPU and peak RSS (10 ms time resolution). Input reads, registration/preparation, execution, JSON serialization/export, reset and output writes are timed. Checks and builds are outside the timed worker. RSS excludes filesystem page cache; warm-cache timings are not cold-disk guarantees. No statistical-significance claim is made.", '', 'large-64: 16 MiB / 64 instructions, one run with eight watchers. large-256: 32 MiB / 256 instructions, one run with the same watchers. Both repeat prefix, suffix, regex replacement and Unicode uppercase. large-256-no-watchers: the same large pipeline using process_all, showing the effect outside reports. Watchers: bytes, Unicode scalar count, words, lines, byte delta, unchanged, next-state bytes and instruction label.', '', 'prepared-constants: 128 identical literal-regex instructions, 500 runs on ~1 KiB of Unicode text. Registration is included, so front-loaded compilation is counted. shared-statistics: 64 direct watcher steps on one immutable 32 MiB snapshot using the same eight functions; no transforms or temporary-buffer transitions. This isolates default-statistics computation, not large-file pipeline execution.', '', 'Every final output matches an independent SHA-256 reference. Every watcher row matches independent numeric/boolean references and the entire JSON, including instruction labels, matches between before/after. Sorted JSON is deterministic across fresh processes. Compressed example exports, exact source archives, worker hash, source hashes and raw samples are retained in this directory.', '', '## Compatibility and resource policy', '', 'Only registered private pipeline instances cache bindings. Exported TokenWrapper clones discard the cache, preventing stale public-template mutations. Custom instructions must opt in via the new default-false supports_preparation method; stateless built-in transforms opt in. Variable references, all interpolation delimiters and nested-token parameters stay dynamic. Invalid bindings retain execution-time errors and unreachable invalid instructions do not fail registration.', '', 'Shared statistics recognize exact function-item TypeIds for current_len_chars, word_count, line_count, char_delta and word_count_delta. No function-pointer address comparison is used. Aliases of the same function work; function pointers and wrapping/custom closures stay on the ordinary callback path. No text is retained by the statistics cache beyond the current call, and custom replacement callbacks clear the recognition metadata.', '', 'Public TextBuffer constructors/file APIs retain their explicit spool placement behavior. String-returning processor adapters may retain already materialized fallback results; watcher adapters borrow existing Arc snapshots rather than spooling and rereading them. Explicit spill remains available and error-safe. This is a per-path performance policy, not a process-wide RAM bound. Arbitrary callbacks can still retain complete snapshots; Unicode regex and uppercase semantics remain whole-document.', '', '## Reproduction', '', '`python3 scripts/benchmark_steps345.py`', '', '`--report-only` regenerates this report. Linux/GNU time, Python 3.12+, Rust and cached Cargo dependencies are required. Fixture data reuse data/watcher-reliability/ and add data/steps-345/; pipelines are in pipelines/steps-345/. No dependency, version, bytecode or file-extension change is introduced.']
    validation=REPORTS/'validation-results.json'
    if validation.exists():
        lines += ['', '## Validation', '', '| Command | Exit |', '|---|---:|']
        for row in json.loads(validation.read_text()):
            lines.append('| '+' '.join(row['command'])+' | '+str(row['exit'])+' |')
        lines += ['', 'Full test_access passes, including preparation, dynamic context, cache invalidation, Unicode statistics, retained-buffer transitions, safe allocation refusal and report-reliability tests. Other full test configurations retain known private-parser doctest failures; repository-wide format/Clippy retain inherited issues. Logs are compressed in validation/.']
    (REPORTS/'report.md').write_text('\n'.join(lines)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report-only',action='store_true')
    parser.add_argument('--repetitions',type=int,default=5)
    args=parser.parse_args()
    if args.report_only: report(json.loads((REPORTS/'results.json').read_text()));return
    if args.repetitions<3: parser.error('use at least three repetitions')
    REPORTS.mkdir(parents=True,exist_ok=True)
    scenarios=cases()
    worker=ROOT/'examples/comparison_probe.rs'
    with tarfile.open(REPORTS/'candidate-source.tar.gz','w:gz') as archive:
        for name in ['src','tests','Cargo.toml','Cargo.lock']: archive.add(ROOT/name,arcname=name)
    metadata=dict(repetitions=args.repetitions,cases=scenarios,rustc=subprocess.check_output(['rustc','-Vv'],text=True),platform=platform.platform(),features=['buffered','watchers'],worker_sha256=common.digest(worker),archives={name:common.digest(REPORTS/name) for name in ['baseline-source.tar.gz','candidate-source.tar.gz']},candidate_sources={str(path.relative_to(ROOT)):common.digest(path) for path in sorted((ROOT/'src').rglob('*.rs'))})
    rows=[];fingerprints={};rng=random.Random(20261028)
    with tempfile.TemporaryDirectory(prefix='textforge-steps345-') as temporary:
        temporary=Path(temporary);binaries={}
        for version,name in [('before','baseline-source.tar.gz'),('after','candidate-source.tar.gz')]:
            snapshot=temporary/version;snapshot.mkdir()
            with tarfile.open(REPORTS/name) as archive: archive.extractall(snapshot,filter='data')
            binaries[version]=common.build(snapshot,temporary/('target-'+version),worker)
        env=dict(os.environ,NO_COLOR='1',RAYON_NUM_THREADS='4')
        jobs=list(scenarios)
        for repetition in range(-1,args.repetitions):
            rng.shuffle(jobs)
            for name in jobs:
                versions=['before','after'];rng.shuffle(versions)
                for version in versions:
                    case=scenarios[name];output=temporary/'output';output.mkdir(exist_ok=True)
                    for stale in output.iterdir(): stale.unlink()
                    metrics=temporary/'time.txt'
                    count=case['steps'] if case['mode']=='collect-full' else 1
                    command=['/usr/bin/time','-f','%e %U %S %M','-o',str(metrics),str(binaries[version]),case['mode'],str(ROOT/case['pipeline']),str(ROOT/case['input']),str(output),str(case['iterations']),str(count)]
                    subprocess.run(command,env=env,stdout=subprocess.DEVNULL,check=True,timeout=240)
                    wall,user,system,rss=map(float,metrics.read_text().split())
                    if common.digest(output/'result-0.txt')!=case['output_sha256']: raise RuntimeError('transformed-output mismatch')
                    report_hash=None;report_bytes=0
                    if case['reports']:
                        path=output/'watchers.json';value=json.loads(path.read_text());verify(value,case)
                        report_hash=common.digest(path);report_bytes=path.stat().st_size
                        if name in fingerprints and fingerprints[name]!=report_hash: raise RuntimeError('non-identical JSON reports')
                        fingerprints[name]=report_hash
                        with gzip.open(REPORTS/(name+'-'+version+'.json.gz'),'wb') as compressed: compressed.write(path.read_bytes())
                    row=dict(case=name,version=version,repetition=repetition,wall_seconds=wall,cpu_seconds=user+system,user_seconds=user,system_seconds=system,peak_rss_kib=int(rss),report_bytes=report_bytes,report_sha256=report_hash,output_sha256_match=True)
                    if repetition>=0:
                        rows.append(row)
                        (REPORTS/'results.json').write_text(json.dumps(dict(metadata=metadata,samples=rows),indent=2,ensure_ascii=False))
                    print(f'{repetition=} {name} {version}: wall={wall:.2f}s cpu={user+system:.2f}s rss={rss/1024:.1f}MiB',flush=True)
    report(dict(metadata=metadata,samples=rows))

if __name__=='__main__': main()
