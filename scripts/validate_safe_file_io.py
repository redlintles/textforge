#!/usr/bin/env python3
"""Run and retain the complete AGENTS.md pre-commit checklist, including failures."""
import gzip
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / 'docs/benchmarks/safe-file-io'
COMMANDS = [
    ['cargo', 'fmt', '--check'],
    ['cargo', 'clippy', '--all-targets', '--', '-D', 'warnings'],
    ['cargo', 'clippy', '--all-targets', '--features', 'test_access', '--', '-D', 'warnings'],
    ['cargo', 'test'],
    ['cargo', 'test', '--features', 'bytecode'],
    ['cargo', 'test', '--features', 'watchers'],
    ['cargo', 'test', '--features', 'test_access'],
]


def main():
    phase = sys.argv[1] if len(sys.argv) > 1 else 'pre-commit'
    logs = REPORT / 'validation' / phase
    logs.mkdir(parents=True, exist_ok=True)
    results = []
    environment = dict(os.environ, CARGO_NET_OFFLINE='true')
    validation_root = Path(os.environ.get('TEXTFORGE_VALIDATION_ROOT', str(ROOT)))
    for index, command in enumerate(COMMANDS):
        log = logs / f'{index}.log'
        with log.open('wb') as output:
            result = subprocess.run(command, cwd=validation_root, env=environment, stdout=output, stderr=subprocess.STDOUT)
        compressed = log.with_suffix('.log.gz')
        with gzip.open(compressed, 'wb') as output:
            output.write(log.read_bytes())
        log.unlink()
        results.append(dict(command=command, exit=result.returncode, log=str(compressed.relative_to(ROOT))))
        print(phase, ' '.join(command), result.returncode, flush=True)
    saved = REPORT / 'validation-results.json'
    history = json.loads(saved.read_text()) if saved.exists() else {}
    history[phase] = results
    saved.write_text(json.dumps(history, indent=2) + '\n')


if __name__ == '__main__':
    main()
