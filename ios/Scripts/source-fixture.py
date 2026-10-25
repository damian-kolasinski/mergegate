#!/usr/bin/env python3
"""Seed source gaps on both sides in an isolated smoke hub, using real git blobs."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

run('git', 'switch', 'main')
lines = [f'// Pricing policy {n:02}: delivery is charged once.' for n in range(1, 81)]
lines[0] = '// Checkout pricing — original source context'
lines[4] = 'let explanation = "' + ('Delivery applies to every region; ' * 7) + 'é 🐙"'
lines[19] = 'let total = subtotal'
lines[59] = 'let receiptTotal = subtotal'
(repo / 'checkout.swift').write_text('\n'.join(lines) + '\n')
(repo / 'old-name.txt').write_text('Unchanged renamed source é 🐙')
(repo / 'removed.txt').write_text('Deleted source\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Seed checkout source context')
run('git', 'switch', '-c', 'read-source-context')
lines[19] = 'let total = subtotal + delivery'
lines[59] = 'let receiptTotal = subtotal + delivery'
(repo / 'checkout.swift').write_text('\n'.join(lines) + '\n')
run('git', 'mv', 'old-name.txt', 'renamed.txt')
(repo / 'added.txt').write_text('Added source é 🐙')
(repo / 'removed.txt').unlink()
run('git', 'add', '.')
run('git', 'commit', '-m', 'Include delivery in checkout and receipt totals')
guide = root / 'source-guide.md'
guide.write_text('''Include delivery in both totals. Read surrounding policies before reviewing the calculation.

## Files

- `checkout.swift` — Compare checkout and receipt calculations in their source context.
- `renamed.txt` — The renamed source has no final newline.
- `added.txt` — Added source.
- `removed.txt` — Deleted source.
''')
submitted = json.loads(run(mg, '--json', 'submit', '--title', 'Read checkout source context', '--guide', str(guide)))
(root / 'source.json').write_text(json.dumps(submitted, indent=2))
print(json.dumps(submitted, indent=2))
