#!/usr/bin/env python3
"""Reproducible Linux probe; limits apply only to children, not the host."""
import argparse, hashlib, json, pathlib, resource, signal, subprocess, time, os
p = argparse.ArgumentParser()
p.add_argument('--probe', type=pathlib.Path, required=True)
p.add_argument('--directory', type=pathlib.Path, required=True)
p.add_argument('--pressure-only', action='store_true')
a = p.parse_args()
D = a.directory
D.mkdir(parents=True, exist_ok=True)
(D / 'spill').mkdir(exist_ok=True)
(D / 'batch').mkdir(exist_ok=True)
rows = json.loads((D / 'results.json').read_text()) if a.pressure_only else []

def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()

def reference(path, operation):
    h = hashlib.sha256()
    if operation == 'fallback':
        return hashlib.sha256(path.read_bytes().replace(b'alice', b'[USER]').upper()).hexdigest()
    if operation == 'wrap':
        h.update(b'<')
    for _ in range(4 if operation == 'repeat' else 1):
        with path.open('rb') as f:
            for chunk in iter(lambda: f.read(1024 * 1024), b''):
                h.update(chunk)
    if operation == 'wrap':
        h.update(b'>')
    return h.hexdigest()
if not a.pressure_only:
    for size in [1, 16, 64, 256]:
        block = (b'2026-10-27 INFO user=alice payload=banana\n' * 30000)[:1024 * 1024]
        with (D / f'{size}MiB.txt').open('wb') as f:
            for _ in range(size):
                f.write(block)
    for name, text in [('noop', 'null;\n'), ('wrap', 'atb "<";\nate ">";\n'), ('repeat', 'rpt 4;\n'), ('fallback', 'raw alice "[USER]";\ntua;\n')]:
        (D / f'{name}.textforge').write_text(text)
    unit = 'João é 👩🏽\u200d💻 🇧🇷\n'.encode()
    (D / 'unicode.txt').write_bytes(unit * 1000000)

def run(operation, size, mode, rep=0, batch=1, threshold=1024 * 1024, cap=2 * 1024 ** 3):
    inp = D / f'{size}MiB.txt' if isinstance(size, int) else D / 'unicode.txt'
    expected = reference(inp, operation)
    out = D / 'out.txt' if batch == 1 else D / 'batch'
    rss = D / 'rss'
    rss.unlink(missing_ok=True)
    if batch > 1:
        for old in out.glob('result-*.txt'):
            old.unlink()
    cmd = ['/usr/bin/time', '-f', '%M', '-o', str(rss), str(a.probe.resolve()), mode, str(D / f'{operation}.textforge'), str(inp), str(out), str(D / 'spill'), str(threshold), str(batch)]

    def constrained():
        resource.setrlimit(resource.RLIMIT_AS, (cap, cap))
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    start = time.monotonic()
    process = subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, preexec_fn=constrained, start_new_session=True)
    try:
        _, err = process.communicate(timeout=45)
        code = process.returncode
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        _, err = process.communicate()
        code = 'timeout'
    elapsed = time.monotonic() - start
    match = all((digest(out / f'result-{i}.txt') == expected for i in range(batch))) if code == 0 and batch > 1 else digest(out) == expected if code == 0 else None
    row = {'operation': operation, 'input': str(inp.name), 'input_bytes': inp.stat().st_size, 'mode': mode, 'rep': rep, 'batch': batch, 'threshold': threshold, 'address_space_cap': cap, 'exit': code, 'seconds': round(elapsed, 6), 'rss_kib': rss.read_text().strip() if rss.exists() else '', 'sha256_match': match, 'stderr': err.decode(errors='replace')[:1000]}
    rows.append(row)
    (D / 'results.json').write_text(json.dumps(rows, indent=2, ensure_ascii=False))
    print(row, flush=True)
if not a.pressure_only:
    for size in [64, 256]:
        for operation in ['noop', 'wrap']:
            for mode in ['legacy', 'buffered']:
                for rep in range(3):
                    run(operation, size, mode, rep)
    for operation in ['repeat', 'fallback']:
        for mode in ['legacy', 'buffered', 'buffered-string']:
            run(operation, 64, mode)
    for mode in ['legacy', 'buffered']:
        run('wrap', 'unicode', mode)
    for count in [4, 16]:
        for mode in ['legacy-batch', 'buffered-batch']:
            run('wrap', 16, mode, batch=count)
    for mode in ['legacy', 'buffered', 'buffered-string']:
        run('wrap', 64, mode, cap=96 * 1024 ** 2)
    for mode in ['legacy', 'buffered']:
        run('wrap', 1, mode, threshold=8 * 1024 ** 2)
for mode in ['legacy', 'buffered', 'buffered-string']:
    run('repeat', 64, mode, cap=96 * 1024 ** 2)
print('successful output hash mismatches:', sum((r['sha256_match'] is False for r in rows)), flush=True)
assert not any((r['sha256_match'] is False for r in rows))
