#!/usr/bin/env python3
"""Run a disposable Tailscale hub with a question and two external rejections.

Uses testkit's GitHub fixture for external provenance; every app request and
feedback event passes through the real hub. Keep this process running for tests.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import urllib.parse
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--fake-github', type=Path, required=True,
                    help='testkit fake-github binary (built by the web test suite)')
args = parser.parse_args()
root = args.root.resolve()
root.mkdir(parents=True, exist_ok=False)
mg = os.environ.get('MG_BIN') or shutil.which('mg')
github = subprocess.Popen([str(args.fake_github.resolve())], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, text=True)
ready = json.loads(github.stdout.readline())
env = dict(os.environ, **ready['env'], MERGEGATE_HOME=str(root / 'hub'))

def run(*parts, cwd=root):
    result = subprocess.run(parts, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout.strip()

def command(*parts, cwd=root):
    return json.loads(run(mg, '--json', *parts, cwd=cwd))

def request(base, path, body):
    req = urllib.request.Request(base + '/' + urllib.parse.quote(path, safe='/'),
                                 data=json.dumps(body).encode(),
                                 headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'})
    with urllib.request.urlopen(req) as response:
        return json.load(response)

def stop(*_):
    raise SystemExit(0)

signal.signal(signal.SIGTERM, stop)
signal.signal(signal.SIGINT, stop)
try:
    command('hub', 'start', '--port', '0')
    listening = command('hub', 'set', 'listen', 'tailnet')
    (root / 'listening.json').write_text(json.dumps(listening, indent=2))
    port = listening['tailnet']['url'].rsplit(':', 1)[1]
    base = f'http://127.0.0.1:{port}/api'
    repo = root / 'repo'
    repo.mkdir()
    run('git', 'init', '-b', 'main', cwd=repo)
    (repo / 'receipt.swift').write_text('let total = subtotal\n')
    run('git', 'add', '.', cwd=repo)
    run('git', 'commit', '-m', 'Seed receipt', cwd=repo)
    run('git', 'remote', 'add', 'origin', 'git@github.com:acme/app.git', cwd=repo)
    bare = Path(ready['repos']) / 'acme' / 'app.git'
    bare.parent.mkdir(parents=True, exist_ok=True)
    run('git', 'clone', '--bare', str(repo), str(bare))
    command('project', 'add', str(repo), '--backend', 'github', '--name', 'app', '--languages', 'none')
    command('init', cwd=repo)
    run('git', 'switch', '-c', 'receipt-delivery', cwd=repo)
    (repo / 'receipt.swift').write_text('let total = subtotal + delivery\n')
    run('git', 'add', '.', cwd=repo)
    run('git', 'commit', '-m', 'Include delivery in receipt totals', cwd=repo)
    item = command('submit', '--title', 'Include delivery in receipt totals', cwd=repo)['item']
    item_id = item['id']
    question = request(base, f'items/{item_id}/drafts', {
        'revision': 1, 'path': 'receipt.swift', 'side': 'new', 'start_line': 1, 'line': 1,
        'body': 'Keep the receipt consistent with the charged amount.'})
    request(base, f'items/{item_id}/reviews', {
        'revision': 1, 'verdict': 'changes_requested', 'summary': 'Confirm receipt compatibility.'})
    command('reply', question['id'], '--status', 'question', '--body',
            'Which client needs the final total including delivery?', cwd=repo)
    for body in ['Extract a separate delivery calculation.', 'Add another receipt cache.']:
        request(ready['web'], '_fake/comments', {
            'repo': 'acme/app', 'pull': 1, 'kind': 'issue', 'login': 'reviewer', 'body': body, 'bot': False})
    request(base, f'items/{item_id}/sync', {})
    with urllib.request.urlopen(base + '/items/' + urllib.parse.quote(item_id, safe='')) as response:
        detail = json.load(response)
    external = [comment for comment in detail['comments'] if comment.get('external')]
    for comment in external:
        request(base, f"comments/{comment['id']}/vote", {'vote': 'up'})
    request(base, f'items/{item_id}/sync', {})
    for comment, reason in zip(external, [
            'The shared expression keeps the receipt and charge identical.',
            'The existing cache already holds the final charged total.']):
        command('reply', comment['id'], '--status', 'rejected', '--body', reason, cwd=repo)
    metadata = {'url': listening['tailnet']['url'], 'item': item_id, 'question': question['id'],
                'override': external[0]['id'], 'accept': external[1]['id']}
    (root / 'decisions.json').write_text(json.dumps(metadata, indent=2))
    print(json.dumps(metadata), flush=True)
    signal.pause()
finally:
    subprocess.run([mg, 'hub', 'stop'], env=env, capture_output=True)
    github.stdin.close()
    github.wait(timeout=10)
