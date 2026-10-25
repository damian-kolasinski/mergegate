#!/usr/bin/env python3
"""Seed parity controls on an isolated smoke hub; never uses the operator hub."""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import urllib.parse
import urllib.request
import zlib

root = Path(sys.argv[1]).resolve()
listening = json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

def command(*args):
    return json.loads(run(mg, '--json', *args))

def seed(path, payload):
    port = listening['tailnet']['url'].rsplit(':', 1)[1]
    request = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}',
        data=json.dumps(payload).encode(),
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'})
    with urllib.request.urlopen(request) as response:
        return json.load(response)

def png(color):
    def chunk(name, data):
        return struct.pack('!I', len(data)) + name + data + struct.pack('!I', zlib.crc32(name + data))
    pixels = b''.join(b'\0' + bytes(color) * 64 for _ in range(64))
    return b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', 64, 64, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b'')

if '--verify' in sys.argv[2:]:
    item = json.loads((root / 'parity.json').read_text())['item']
    port = listening['tailnet']['url'].rsplit(':', 1)[1]
    def read(path):
        request = urllib.request.Request(
            f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}',
            headers={'X-Mergegate-Client': 'mergegate-ios'})
        with urllib.request.urlopen(request) as response:
            return json.load(response)
    value = read(f'items/{item}')
    assert value['item']['state'] == 'abandoned', value['item']
    assert value['item']['priority'] == 'p0', value['item']
    assert any(comment['body'] == 'Hub draft retained for editing.' for comment in value['comments'])
    assert any(reply['author'] == 'operator' and reply['body'] == 'The native preview shows the table.'
               for reply in value['replies'])
    assert not value['drafts'] and not value['draft_replies']
    rules = read('projects/mobile-smoke/rules')
    assert any(rule['tier'] == 'trusted' and rule['reason'] == 'Generated fixture' for rule in rules['rules'])
    print(json.dumps({'item': item, 'state': 'abandoned', 'priority': 'p0',
                      'feedback_delivered': True, 'rules_applied': True}, indent=2))
    raise SystemExit()

# Keep the common smoke layout's item numbers stable for all UI verification scripts.
subprocess.run([sys.executable, str(Path(__file__).with_name('conversations-fixture.py')), str(root)], check=True)
run('git', 'switch', 'main')
(repo / 'preview.png').write_bytes(png((30, 60, 120)))
run('git', 'add', '.')
run('git', 'commit', '-m', 'Seed image comparison')
run('git', 'switch', '-c', 'native-parity')
(repo / 'preview.png').write_bytes(png((60, 180, 120)))
(repo / 'README.md').write_text('# Native parity\n\nReadable **Markdown** and a table.\n\n| Before | After |\n| --- | --- |\n| 1 | 2 |\n')
(repo / 'generated.lock').write_text('fixture-version=2\n')
run('git', 'add', '.')
run('git', 'commit', '-m', 'Verify native review controls')
guide = root / 'parity-guide.md'
guide.write_text('''Make review controls available on this phone.

## Files

- `README.md` — Read the rendered explanation.
- `preview.png` — Compare the pixels.
- `generated.lock` — Generated fixture for a proposed rule.
''')
submitted = command('submit', '--title', 'Native parity verification', '--guide', str(guide))
item = submitted['item']['id']
command('attach', item, str(repo / 'preview.png'))
# This proposal is disposable test data for the requested rule-approval UI verification.
proposal = command('rules', 'propose', '--tier', 'trusted', '--path', '*.lock', '--reason', 'Generated fixture', '--project', 'mobile-smoke')
comment = seed(f'items/{item}/drafts', dict(revision=1, path='README.md', side='new', start_line=1, line=1, body='Check the native explanation.'))
seed(f'items/{item}/reviews', dict(revision=1, verdict='comment', summary='Verify the native controls.'))
command('reply', comment['id'], '--status', 'question', '--body', 'Does the native preview show the table?')
draft = seed(f'items/{item}/drafts', dict(revision=1, path='README.md', side='new', start_line=3, line=3, body='Hub draft retained for editing.'))
command('prioritize', item, '--priority', 'p1', '--note', 'Verify on device')
(root / 'parity.json').write_text(json.dumps({'item': item, 'comment': comment, 'draft': draft, 'proposal': proposal}, indent=2))
print(json.dumps({'item': item, 'comment': comment['id'], 'draft': draft['id'], 'url': listening['tailnet']['url']}, indent=2))
