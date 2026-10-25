#!/usr/bin/env python3
"""Seed cached source and drive outages/revisions on a disposable, real hub."""
import argparse
from http.server import BaseHTTPRequestHandler, HTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--pack', required=True, type=Path)
args = parser.parse_args()
root = args.root.resolve()
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')
subprocess.run([sys.executable, str(Path(__file__).with_name('highlight-fixture.py')),
                str(root), '--pack', str(args.pack)], check=True)

def run(*parts):
    return subprocess.check_output(parts, cwd=repo, env=env, text=True).strip()

run(mg, 'lang', 'add', '--from', str(root / 'swift.tar.gz'))
port = int(json.loads((root / 'listening.json').read_text())['tailnet']['url'].rsplit(':', 1)[1])
item = json.loads((root / 'source.json').read_text())['item']['id']

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        try:
            if self.path == '/pause':
                value = json.loads(run(mg, '--json', 'hub', 'stop'))
            elif self.path == '/resume-advance':
                run(mg, 'hub', 'start', '--port', str(port))
                path = repo / 'checkout.swift'
                path.write_text(path.read_text().replace('let total = subtotal + delivery',
                    'let total = subtotal + delivery // Check the receipt, revision 2'))
                run('git', 'add', '.')
                run('git', 'commit', '-m', 'Clarify receipt verification')
                value = json.loads(run(mg, '--json', 'submit', '--title', 'Read checkout source context'))
                assert value['item']['revision']['number'] == 2
            elif self.path in ['/verify-stale', '/verify-sent']:
                with urllib.request.urlopen(f'http://127.0.0.1:{port}/api/events?since=0') as response:
                    events = json.load(response)['events']
                reviews = [event for event in events if event.get('item') == item and event['type'] == 'review.submitted']
                if self.path == '/verify-stale':
                    assert not reviews, reviews
                else:
                    assert len(reviews) == 1, reviews
                    review = reviews[0]
                    assert review['revision'] == 2, review
                    assert review['summary'] == 'Current revision checked after reconnect.', review
                    assert not review['comments'], review
                value = {'verified': True}
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
(root / 'offline-control.json').write_text(json.dumps({'url': url}))
print(url, flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    pass
finally:
    server.server_close()
