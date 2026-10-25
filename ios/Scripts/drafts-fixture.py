#!/usr/bin/env python3
"""Seed both diff sides for persistent drafts on a disposable smoke hub."""
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
