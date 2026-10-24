#!/usr/bin/env python3
"""Create a real, disposable local-backend review item on a Tailscale hub."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('action', choices=['start', 'stop'])
parser.add_argument('root', type=Path)
args = parser.parse_args()
root = args.root.resolve()
root.mkdir(parents=True, exist_ok=True)
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*command, cwd=root):
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

def command(*parts, cwd=root):
    return json.loads(run(mg, '--json', *parts, cwd=cwd))

if args.action == 'stop':
    print(json.dumps(command('hub', 'stop')))
    raise SystemExit()

repo = root / 'repo'
if repo.exists():
    raise SystemExit('Use a fresh directory for each verification run.')
repo.mkdir()
try:
    run('git', 'init', '-b', 'main', cwd=repo)
    (repo / 'README.md').write_text('# Mobile verification\n')
    run('git', 'add', 'README.md', cwd=repo)
    run('git', 'commit', '-m', 'Initialize mobile verification repository', cwd=repo)
    command('hub', 'start', '--port', '0')
    listening = command('hub', 'set', 'listen', 'tailnet')
    (root / 'listening.json').write_text(json.dumps(listening, indent=2))
    command('project', 'add', str(repo), '--name', 'mobile-smoke', '--backend', 'local', '--target', 'main', '--languages', 'none')
    command('init', cwd=repo)
    run('git', 'switch', '-c', 'connect-reopen', cwd=repo)
    (repo / 'README.md').write_text('# Mobile verification\n\nConnect and reopen a real inbox item.\n')
    run('git', 'add', 'README.md', cwd=repo)
    run('git', 'commit', '-m', 'Connect and reopen verification', cwd=repo)
    submitted = command('submit', '--title', 'Connect and reopen verification', cwd=repo)
    (root / 'inbox.json').write_text(json.dumps(command('inbox'), indent=2))
    print(json.dumps({'root': str(root), 'listening': listening, 'submitted': submitted}, indent=2))
except BaseException:
    subprocess.run([mg, 'hub', 'stop'], env=env, capture_output=True)
    raise
