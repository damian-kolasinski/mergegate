#!/usr/bin/env python3
"""Submit two real revisions and verify hub carryover on an isolated smoke hub."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request

root = Path(sys.argv[1]).resolve()
listening = json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')
port = listening['tailnet']['url'].rsplit(':', 1)[1]

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

def request(path, payload=None, method=None):
    req = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/?=")}',
        data=json.dumps(payload).encode() if payload is not None else None,
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'}, method=method)
    with urllib.request.urlopen(req) as response:
        body = response.read()
        return json.loads(body) if body else None

run('git', 'switch', 'main')
(repo / 'checkout.swift').write_text('let subtotal = 100\nlet delivery = 12\nlet total = subtotal\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Seed checkout calculation')
run('git', 'switch', '-c', 'rereview-checkout')
(repo / 'checkout.swift').write_text('let subtotal = 100\nlet delivery = 12\nlet total = subtotal + delivery\n')
(repo / 'policy.txt').write_text('Delivery is charged once per checkout.\n')
(repo / 'obsolete.txt').write_text('Temporary delivery note.\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Include delivery in checkout totals')
guide = root / 'rereview-guide.md'
guide.write_text('''Include delivery in the displayed total. Recheck the changed fee; retain the reviewed policy.

## Files

- `checkout.swift` — Check the amount charged, including the delivery fee.
- `policy.txt` — Confirm the delivery policy.
- `obsolete.txt` — Temporary delivery note.
''')
submitted = json.loads(run(mg, '--json', 'submit', '--title', 'Re-review delivery and receipt totals', '--guide', str(guide)))
item = submitted['item']['id']
comments = []
for path, line, body in [
    ('checkout.swift', 3, 'Keep the charged total consistent with the receipt.'),
    ('checkout.swift', 2, 'Confirm the new delivery fee.'),
    ('obsolete.txt', 1, 'Remove the temporary delivery note.'),
]:
    comments.append(request(f'items/{item}/drafts', dict(
        revision=1, path=path, side='new', start_line=line, line=line, body=body)))
for path in ['checkout.swift', 'policy.txt', 'obsolete.txt']:
    request(f'items/{item}/viewed', dict(revision=1, path=path, viewed=True), 'PUT')
request(f'items/{item}/reviews', dict(revision=1, verdict='changes_requested', summary='Recheck delivery and remove the temporary note.'))
run(mg, 'reply', comments[0]['id'], '--status', 'addressed')
(repo / 'checkout.swift').write_text('// Receipt total includes delivery.\nlet subtotal = 100\nlet delivery = 15\nlet total = subtotal + delivery\n')
(repo / 'obsolete.txt').unlink()
run('git', 'add', '.')
run('git', 'commit', '-m', 'Update delivery fee and remove temporary note')
guide.write_text(guide.read_text().replace('- `obsolete.txt` — Temporary delivery note.\n', ''))
revision = json.loads(run(mg, '--json', 'submit', '--title', 'Re-review delivery and receipt totals', '--guide', str(guide)))
comparison = request(f'items/{item}?since=1')
assert comparison['item']['revision']['number'] == 2
assert comparison['last_reviewed'] == 1
files = {file['path']: file for file in comparison['files']}
assert files['policy.txt']['viewed'] and not files['checkout.swift']['viewed']
assert files['obsolete.txt']['interdiff'] == 'dropped'
conversation = request(f'items/{item}/conversation?onto=2')
assert [comment['placement']['kind'] for comment in conversation['comments']] == ['moved', 'changed', 'outdated']
(root / 'rereview.json').write_text(json.dumps(dict(item=item, comparison=comparison, conversation=conversation), indent=2))
print(json.dumps(dict(item=item, revision=revision, placements=[c.get('placement') for c in conversation['comments']]), indent=2))
