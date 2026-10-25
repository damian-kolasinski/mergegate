#!/usr/bin/env python3
"""Seed both diff sides for persistent drafts on a disposable smoke hub."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--pack', type=Path, help='Installed Swift pack directory; omit for plain-text fallback')
args = parser.parse_args()
root = args.root.resolve()
json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

if args.pack:
    pack = args.pack.resolve()
    if not (pack / 'pack.json').is_file() or not (pack / 'swift.wasm').is_file():
        raise SystemExit('Pass an installed Swift pack directory containing pack.json and swift.wasm.')
    archive = root / 'swift.tar.gz'
    with tarfile.open(archive, 'w:gz') as target:
        for entry in sorted(pack.iterdir()):
            if entry.is_file() and not entry.name.startswith('.'):
                target.add(entry, arcname=entry.name)
    run(mg, 'lang', 'add', '--from', str(archive))

run('git', 'switch', 'main')
(repo / 'checkout.swift').write_text('let subtotal = 100\nlet delivery = 12\nlet total = subtotal\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Seed checkout calculation')
run('git', 'switch', '-c', 'keep-comment-drafts')
(repo / 'checkout.swift').write_text('let subtotal = 100\nlet delivery = 12\nlet total = subtotal + delivery // Delivery is charged once per checkout; preserve Unicode é 🐙\n')
(repo / 'policy.txt').write_text('Delivery is charged once per checkout.\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Include delivery in checkout totals')
guide = root / 'drafts-guide.md'
guide.write_text('''Include delivery in the displayed total. Review the old and new calculation before leaving feedback.

## Files

- `checkout.swift` — Check the amount charged, including the delivery fee.
- `policy.txt` — Confirm the delivery policy.
''')
submitted = json.loads(run(mg, '--json', 'submit', '--title', 'Include delivery in checkout totals', '--guide', str(guide)))
(root / 'drafts.json').write_text(json.dumps(submitted, indent=2))
print(json.dumps(submitted, indent=2))
