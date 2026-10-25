#!/usr/bin/env python3
"""Drive real CLI changes and hub outages while the native live-update flow runs."""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.parse
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path, help='Fresh smoke-hub.py directory')
parser.add_argument('--pack', type=Path, help='Optional installed Swift grammar directory')
args = parser.parse_args()
root = args.root.resolve()
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')
seed_command = [sys.executable, str(Path(__file__).with_name('drafts-fixture.py')), str(root)]
if args.pack:
    seed_command += ['--pack', str(args.pack)]
subprocess.run(seed_command, check=True)
port = int(json.loads((root / 'listening.json').read_text())['tailnet']['url'].rsplit(':', 1)[1])
item = json.loads((root / 'drafts.json').read_text())['item']['id']
revision = 1
new_items = 0

def run(*parts):
    return subprocess.check_output(parts, cwd=repo, env=env, text=True).strip()

def seed(path, payload):
    request = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}',
        data=json.dumps(payload).encode(),
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'})
    with urllib.request.urlopen(request) as response:
        return json.load(response)

comment = seed(f'items/{item}/drafts', dict(revision=1, path='checkout.swift', side='old',
               start_line=3, line=3, body='Should the receipt include delivery in this total?'))
seed(f'items/{item}/reviews', dict(revision=1, verdict='comment', summary='Check delivery on the receipt.'))

def advance():
    global revision
    revision += 1
    (repo / 'receipt.txt').write_text(
        f'The receipt includes delivery, revision {revision}.\n'
        'Delivery is charged once per checkout.\n'
        'Keep the receipt consistent with the final charged total.\n'
        'Preserve the delivery charge on the customer copy.\n'
        'Confirm the receipt before sending it to the customer.\n')
    run('git', 'add', 'receipt.txt')
    run('git', 'commit', '-m', 'Include delivery on receipts')
    result = json.loads(run(mg, '--json', 'submit', '--title', 'Include delivery in checkout totals'))
    assert result['item']['revision']['number'] == revision
    return result

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        global new_items
        try:
            if self.path == '/new-item':
                new_items += 1
                run('git', 'switch', '-c', f'live-inbox-{new_items}', 'main')
                (repo / f'live-{new_items}.txt').write_text('Arrived while the inbox was open.\n')
                run('git', 'add', '.')
                run('git', 'commit', '-m', 'Add a live inbox item')
                value = json.loads(run(mg, '--json', 'submit', '--title', f'Live inbox arrival {new_items}'))
                run('git', 'switch', 'keep-comment-drafts')
            elif self.path == '/reply':
                value = json.loads(run(mg, '--json', 'reply', comment['id'], '--status', 'addressed',
                    '--body', 'Delivery is included in the receipt. Unicode é 🐙 stays intact.'))
            elif self.path == '/comment':
                value = seed(f'items/{item}/drafts', dict(revision=1, path='policy.txt', side='new',
                    start_line=1, line=1, body='Confirm the delivery policy on the receipt.'))
                seed(f'items/{item}/reviews', dict(revision=1, verdict='comment', summary='Review the delivery policy.'))
            elif self.path == '/advance':
                value = advance()
            elif self.path == '/pause':
                value = json.loads(run(mg, '--json', 'hub', 'stop'))
            elif self.path == '/resume':
                run(mg, 'hub', 'start', '--port', str(port))
                value = advance()
            else:
                self.send_error(404)
                return
            body = json.dumps(value).encode()
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        except Exception as error:
            self.send_error(500, str(error))

server = HTTPServer(('127.0.0.1', 0), Handler)
url = f'http://127.0.0.1:{server.server_port}'
(root / 'live-control.json').write_text(json.dumps({'url': url}))
print(url, flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
