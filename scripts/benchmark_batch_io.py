#!/usr/bin/env python3
"""Compare current non-buffered batch with an isolated safe-read patch."""
import hashlib
import json
import os
from pathlib import Path
import random
import shutil
import statistics
import subprocess
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / 'docs/benchmarks/batch-io'

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    REPORT.mkdir(parents=True, exist_ok=True)
    data = ROOT / 'data/batch-io'
    pipes = ROOT / 'pipelines/batch-io'
    data.mkdir(exist_ok=True)
    pipes.mkdir(exist_ok=True)
    source = ROOT / 'data/watcher-reliability/large-64/input.txt'
    for i in range(4):
        shutil.copyfile(source, data / f'input-{i}.txt')
    (pipes / 'read-copy.tft').write_text('null;\n')
    shutil.copyfile(ROOT / 'pipelines/steps-345/large-64.tft', pipes / 'mixed-64.tft')
    worker = ROOT / 'examples/batch_io_probe.rs'
    metadata = dict(rust=subprocess.check_output(['rustc','--version'],text=True).strip(), repetitions=5, batch_files=4, bytes_per_file=source.stat().st_size, input_sha256=sha(source), worker_sha256=sha(worker), features=[], warm_cache=True)
    samples = []
    with tempfile.TemporaryDirectory(prefix='textforge-batch-io-') as temporary:
        temp = Path(temporary)
        binaries = {}
        for variant in ['mmap','safe-read']:
            snapshot = temp / variant
            snapshot.mkdir()
            shutil.copytree(ROOT / 'src', snapshot / 'src')
            for name in ['Cargo.toml','Cargo.lock']:
                shutil.copyfile(ROOT / name, snapshot / name)
            (snapshot / 'examples').mkdir()
            shutil.copyfile(worker, snapshot / 'examples/batch_io_probe.rs')
            code = snapshot / 'src/api/processor.rs'
            if variant == 'safe-read':
                text = code.read_text()
                start = text.index('                    // mmap(2) refuses', text.index('    #[cfg(not(feature = "buffered"))]\n    fn process_batch'))
                end = text.index('\n                    if\n', start)
                replacement = '''                    let mut bytes = Vec::new();
                    std::io::Read::read_to_end(&mut &file, &mut bytes).map_err(|e| {
                        TextForgeError::new(
                            TextForgeErrorCode::FileReadingError("Failed to read origin file".into()),
                            Cow::Borrowed("process_batch"), format!("{:?} - {}", origin, e))
                    })?;
                    let input = String::from_utf8(bytes).map_err(|e| {
                        TextForgeError::new(
                            TextForgeErrorCode::InvalidUtf8Error("Origin file is not valid UTF-8".into()),
                            Cow::Borrowed("process_batch"), format!("{:?} - {}", origin, e))
                    })?;
                    let output = self.run_transform(pipeline_id, input.into())?;
'''
                code.write_text(text[:start] + replacement + text[end:])
                (REPORT / 'safe-read-replacement.txt').write_text(replacement)
            with tarfile.open(REPORT / f'{variant}-source.tar.gz', 'w:gz') as archive:
                for name in ['src','Cargo.toml','Cargo.lock','examples']:
                    archive.add(snapshot / name, arcname=name)
            env = dict(os.environ, CARGO_TARGET_DIR=str(temp / 'target'))
            with (REPORT / f'build-{variant}.log').open('w') as log:
                subprocess.run(['cargo','build','--offline','--locked','--release','--example','batch_io_probe'], cwd=snapshot, env=env, stdout=log, stderr=log, check=True)
            binaries[variant] = temp / f'probe-{variant}'
            shutil.copy2(temp / 'target/release/examples/batch_io_probe', binaries[variant])
        expected = {}
        raw = source.read_text()
        for case in ['read-copy','mixed-64']:
            output = raw
            if case == 'mixed-64':
                for _ in range(16):
                    output = ('<' + output + '>').replace('alice','[USER]').upper()
            expected[case] = hashlib.sha256(output.encode()).hexdigest()
        rng = random.Random(731)
        for case, iterations in [('read-copy',8),('mixed-64',1)]:
            for threads in [1,4]:
                for repetition in range(-1,5):
                    order = ['mmap','safe-read']
                    rng.shuffle(order)
                    for variant in order:
                        out = temp / 'output'
                        out.mkdir(exist_ok=True)
                        timer = temp / 'time.txt'
                        env = dict(os.environ, RAYON_NUM_THREADS=str(threads))
                        subprocess.run(['/usr/bin/time','-f','%e %U %S %M','-o',str(timer),str(binaries[variant]),str(pipes / f'{case}.tft'),str(data),str(out),str(iterations)],env=env,check=True)
                        wall,user,system,rss = map(float,timer.read_text().split())
                        for i in range(4):
                            if sha(out / f'output-{i}.txt') != expected[case]:
                                raise RuntimeError('output mismatch')
                        if repetition >= 0:
                            row = dict(case=case, threads=threads, variant=variant, repetition=repetition, iterations=iterations, wall_seconds=wall, cpu_seconds=user+system, peak_rss_kib=rss, outputs_verified=True)
                            samples.append(row)
                            print(row,flush=True)
        checks = []
        for label, content, success in [('empty',b'',True),('invalid-utf8',b'\xff',False)]:
            check_dir = temp / label
            check_dir.mkdir()
            for i in range(4): (check_dir / f'input-{i}.txt').write_bytes(content)
            for variant in binaries:
                result = subprocess.run([str(binaries[variant]),str(pipes/'read-copy.tft'),str(check_dir),str(temp/'checked-output'),'1'],capture_output=True,text=True)
                if (result.returncode == 0) != success or (not success and 'InvalidUtf8Error' not in result.stderr):
                    raise RuntimeError(f'edge-case mismatch {label} {result.stderr}')
                checks.append(dict(case=label,variant=variant,passed=True))
    metadata['expected_sha256'] = expected
    metadata['archives_sha256'] = {v:sha(REPORT/f'{v}-source.tar.gz') for v in ['mmap','safe-read']}
    saved = dict(metadata=metadata,samples=samples,checks=checks)
    (REPORT/'results.json').write_text(json.dumps(saved,indent=2)+'\n')
    lines = ['# Batch input: mmap versus safe read', '', 'Both snapshots use the current prepared non-buffered engine (default features). The only production-code difference is the batch input block: mmap + UTF-8 validation + copy versus Read::read_to_end + String::from_utf8. The Vec allocation is transferred into String without another byte copy. process_file still uses mmap in both snapshots. The working implementation and dependencies are unchanged; archives and the exact replacement are retained here.', '', 'Four distinct 16 MiB Unicode files, same contents. read-copy: null pipeline, 8 batch calls (512 MiB read and written per worker). mixed-64: one batch, 64 instructions per file (prefix, suffix, regex replacement, Unicode uppercase). Runs use either one or four Rayon threads. Five fresh-process measurements per variant/case/thread count, randomized paired order, one excluded paired warm-up. Release, offline locked dependencies; Rust '+metadata['rust']+'. GNU time measures startup, pipeline registration, reads, processing and writes. CPU is user+system; peak RSS excludes page cache. Cache is warm, files are not fsynced, and this is not a cold-storage throughput test. Timer resolution is 10 ms. Output hashes match independent Python references for every sample; empty files and invalid UTF-8 are checked in both builds.', '', '| Pipeline | Threads | Variant | Wall median (s) | CPU median (s) | Peak RSS (MiB) | Wall range (s) |', '|---|---:|---|---:|---:|---:|---|']
    for case in ['read-copy','mixed-64']:
        for threads in [1,4]:
            for variant in ['mmap','safe-read']:
                rows=[r for r in samples if r['case']==case and r['threads']==threads and r['variant']==variant]
                med=lambda key:statistics.median(r[key] for r in rows)
                lines.append(f"| {case} | {threads} | {variant} | {med('wall_seconds'):.2f} | {med('cpu_seconds'):.2f} | {med('peak_rss_kib')/1024:.1f} | {min(r['wall_seconds'] for r in rows):.2f}–{max(r['wall_seconds'] for r in rows):.2f} |")
    lines += ['', '## Interpretation', '', 'Removing mmap does not make whole-document transforms streaming and is not a global RAM limit. Control active tasks first (a local Rayon pool, or bounded submission), then consider a byte-budget scheduler using conservative per-pipeline expansion estimates. A single task may retain input/output/intermediate text; regex, JSON, reversal and arbitrary instructions cannot generally be split into independent chunks without changing semantics. TextBuffer can spill compatible operations, but fallbacks and String-returning methods materialize full text. An explicit file-to-file streaming contract would be needed for a compatible subset of transforms.', '', 'RSS of file-backed mappings is charged per process while resident, even though it is reclaimable page-cache-backed memory. Anonymous String allocations are different; lower RSS here does not mean identical reduction in total OS physical memory. mmap remains live throughout run_transform in the baseline; it also copies the complete input before execution. These results do not show an advantage for a zero-copy mmap design, which this engine does not implement.', '', '## Reproduce', '', '`python3 scripts/benchmark_batch_io.py`', '', 'Raw measurements, source snapshots, build logs and edge checks are stored beside this report. No dependency or public API changes were made.']
    (REPORT/'report.md').write_text('\n'.join(lines)+'\n')

if __name__ == '__main__': main()
