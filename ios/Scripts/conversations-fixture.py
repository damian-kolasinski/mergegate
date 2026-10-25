#!/usr/bin/env python3
"""Seed sent threads through real routes on an isolated smoke hub."""
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
subprocess.run([sys.executable, str(Path(__file__).with_name('drafts-fixture.py')), str(root)], check=True)
item = json.loads((root / 'drafts.json').read_text())['item']['id']
port = listening['tailnet']['url'].rsplit(':', 1)[1]
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def seed(path, payload):
    request = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}',
        data=json.dumps(payload).encode(),
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'})
    with urllib.request.urlopen(request) as response:
        return json.load(response)

def command(*args):
    result = subprocess.run([mg, '--json', *args], cwd=root / 'repo', env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return json.loads(result.stdout)

comments = []
for path, side, line, body in [
    ('checkout.swift', 'old', 3, 'Should the receipt include delivery in this total?'),
    ('checkout.swift', 'new', 3, 'Extract a separate delivery calculation.'),
    ('checkout.swift', 'new', 1, 'Keep the subtotal in minor currency units.'),
    ('policy.txt', 'new', 1, 'Keep this policy visible to the receipt client.'),
]:
    comments.append(seed(f'items/{item}/drafts', dict(
        revision=1, path=path, side=side, start_line=line, line=line, body=body)))
seed(f'items/{item}/reviews', dict(revision=1, verdict='changes_requested', summary='Check receipt and delivery consistency.'))
command('reply', comments[0]['id'], '--status', 'question', '--body',
        'Which client still reads the subtotal-only value? Unicode é 🐙 stays intact.')
command('reply', comments[1]['id'], '--status', 'rejected', '--body',
        'This expression is shared with the receipt. Extracting it would hide the charged amount.')
command('reply', comments[2]['id'], '--status', 'addressed')
seed(f"comments/{comments[0]['id']}/drafts", dict(body='The receipt client needs the final charged total.'))
seed(f'items/{item}/reviews', dict(revision=1, verdict='comment', summary='Receipt compatibility confirmed.'))
(root / 'conversations.json').write_text(json.dumps({'item': item, 'comments': comments}, indent=2))
print(json.dumps({'item': item, 'comments': [comment['id'] for comment in comments]}, indent=2))
