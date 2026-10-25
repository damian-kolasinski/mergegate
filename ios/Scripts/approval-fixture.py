#!/usr/bin/env python3
"""Advance a disposable item's revision and verify its real target after native approval."""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path, help='Fresh smoke-hub.py + drafts-fixture.py directory')
args = parser.parse_args()
root = args.root.resolve()
repo = root / 'repo'
assert (root / 'drafts.json').is_file(), 'Seed drafts-fixture.py first'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*parts):
    return subprocess.check_output(parts, cwd=repo, env=env, text=True).strip()

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        if self.path != '/advance':
            self.send_error(404)
            return
        try:
            (repo / 'receipt.txt').write_text('The receipt includes delivery.\n')
            run('git', 'add', 'receipt.txt')
            run('git', 'commit', '-m', 'Include delivery on receipts')
            value = json.loads(run(mg, '--json', 'submit', '--title', 'Include delivery in checkout totals'))
            assert value['item']['revision']['number'] == 2
            self.reply(value)
        except Exception as error:
            self.send_error(500, str(error))

    def do_GET(self):
        if self.path != '/verify':
            self.send_error(404)
            return
        try:
            commit = run('git', 'rev-parse', 'main')
            source = run('git', 'show', 'main:checkout.swift')
            receipt = run('git', 'show', 'main:receipt.txt')
            assert 'subtotal + delivery' in source
            assert receipt == 'The receipt includes delivery.'
            assert run('git', 'rev-parse', 'main^{tree}') == run('git', 'rev-parse', 'HEAD^{tree}')
            self.reply({'commit': commit, 'target': 'main', 'verified': True})
        except Exception as error:
            self.send_error(500, str(error))

    def reply(self, value):
        body = json.dumps(value).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.end_headers()
        self.wfile.write(body)

server = HTTPServer(('127.0.0.1', 0), Handler)
url = f'http://127.0.0.1:{server.server_port}'
(root / 'approval-control.json').write_text(json.dumps({'url': url}))
print(url, flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
