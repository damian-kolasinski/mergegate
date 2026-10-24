#!/usr/bin/env python3
"""Add triage scenarios to a disposable smoke hub; never targets the operator hub."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.request
import urllib.parse
import urllib.error

root = Path(sys.argv[1]).resolve()
# This file is written only by smoke-hub.py, alongside its isolated home/repo.
listening = json.loads((root / 'listening.json').read_text())
port = listening['tailnet']['url'].rsplit(':', 1)[1]
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args, cwd=None):
    result = subprocess.run(args, cwd=cwd or root / 'repo', env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

def command(*args, cwd=None):
    return json.loads(run(mg, '--json', *args, cwd=cwd))

# Seed review actions only in the disposable fixture through its real routes.
def seed(path, payload, method='POST'):
    request = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}', data=json.dumps(payload).encode(),
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'}, method=method)
    with urllib.request.urlopen(request) as response:
        body = response.read()
        return json.loads(body) if body else None

repo = root / 'repo'
run('git', 'switch', 'main')
run('git', 'switch', '-c', 'triage-question')
(repo / 'README.md').write_text('# Mobile verification\n\nKeep the legacy response format.\n')
(repo / 'client.swift').write_text('let responseFormat = "legacy"\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Choose the response format')
item = command('submit', '--title', 'Choose the response format')['item']
item_id = item['id']
comment = seed(f'items/{item_id}/drafts', dict(
    revision=1, path='README.md', side='new', start_line=3, line=3,
    body='Keep compatibility with the previous response format.'))
seed(f'items/{item_id}/reviews', dict(revision=1, verdict='changes_requested', summary='Confirm the compatibility requirement.'))
command('reply', comment['id'], '--status', 'question', '--body', 'Which clients still need the legacy format?')
seed(f'items/{item_id}/viewed', dict(revision=1, path='README.md', viewed=True), method='PUT')
command('prioritize', item_id, '--priority', 'p1', '--note', 'Compatibility decision before release')

run('git', 'switch', 'main')
run('git', 'switch', '-c', 'triage-working')
(repo / 'README.md').write_text('# Mobile verification\n\nRetry failed connections.\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Retry failed connections')
working = command('submit', '--title', 'Retry failed connections')['item']
seed(f"items/{working['id']}/reviews", dict(revision=1, verdict='changes_requested', summary='Handle timeout recovery.'))

other = root / 'other-repo'
other.mkdir()
run('git', 'init', '-b', 'main', cwd=other)
(other / 'README.md').write_text('# Companion service\n')
run('git', 'add', '.', cwd=other)
run('git', 'commit', '-m', 'Initialize companion service', cwd=other)
command('project', 'add', str(other), '--name', 'companion', '--backend', 'local', '--target', 'main', '--languages', 'none')
command('init', cwd=other)
run('git', 'switch', '-c', 'triage-closed', cwd=other)
(other / 'README.md').write_text('# Companion service\n\nRetire the old endpoint.\n')
run('git', 'add', '.', cwd=other)
run('git', 'commit', '-m', 'Retire the old endpoint', cwd=other)
closed = command('submit', '--title', 'Retire the old endpoint', cwd=other)['item']
command('abandon', closed['id'], '--reason', 'Superseded by the new service', cwd=other)
(root / 'triage.json').write_text(json.dumps(command('inbox'), indent=2))
print(json.dumps({'question': item_id, 'working': working['id'], 'closed': closed['id']}))
