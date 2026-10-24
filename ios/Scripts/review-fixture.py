#!/usr/bin/env python3
"""Submit a guided source change to a disposable smoke hub for the review flow."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
# Require the isolated smoke-hub layout before issuing any commands.
json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

run('git', 'switch', '-c', 'start-review', 'main')
(repo / 'z-checkout.swift').write_text('''struct Checkout {
    let subtotal: Int
    let delivery: Int

    var total: Int {
        subtotal + delivery
    }
}
''')
(repo / 'README.md').write_text('# Mobile review verification\n\nInclude delivery in the checkout total.\n')
(repo / 'a-notes.txt').write_text('Delivery is charged once per checkout.\n' + ''.join(
    f'Region {number}: apply the published delivery rate.\n' for number in range(1, 81)))
(repo / 'receipt.bin').write_bytes(b'\x00\xff\x00\x01receipt')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Include delivery in checkout totals')
guide = root / 'review-guide.md'
guide.write_text('''Include delivery in checkout totals so the amount shown matches the amount charged.

Start with the calculation, then check its user-facing explanation.

## Files

- `z-checkout.swift` — Review the total calculation first.
- `README.md` — Check the explanation of the delivery charge.
- `a-notes.txt` — Confirm the delivery policy.
- `receipt.bin` — Binary receipt fixture.
''')
submitted = json.loads(run(mg, '--json', 'submit', '--title', 'Include delivery in checkout totals', '--guide', str(guide)))
(root / 'review.json').write_text(json.dumps(submitted, indent=2))
print(json.dumps(submitted, indent=2))
