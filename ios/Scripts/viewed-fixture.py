#!/usr/bin/env python3
"""Trust the binary receipt only on a disposable smoke hub's review fixture."""
import json
from pathlib import Path
import sys
import urllib.parse
import urllib.request

root = Path(sys.argv[1]).resolve()
listening = json.loads((root / 'listening.json').read_text())
review = json.loads((root / 'review.json').read_text())['item']
if review['backend'] != 'local' or review['id'] != 'mobile-smoke#2':
    raise SystemExit('Use the disposable smoke-hub.py and review-fixture.py layout.')
port = listening['tailnet']['url'].rsplit(':', 1)[1]


def seed(path, payload):
    request = urllib.request.Request(
        f'http://127.0.0.1:{port}/api/{urllib.parse.quote(path, safe="/")}',
        data=json.dumps(payload).encode(),
        headers={'Content-Type': 'application/json', 'X-Mergegate-Client': 'mergegate-ios'}, method='POST')
    with urllib.request.urlopen(request) as response:
        return json.load(response)


# Operator actions here target only the isolated local-backend fixture.
proposal = seed('projects/mobile-smoke/rules/proposals', {
    'tier': 'trusted', 'path': 'receipt.bin', 'reason': 'Binary receipt fixture.'})
rules = seed(f"projects/mobile-smoke/rules/proposals/{proposal['rule']['id']}/approve", {})
(root / 'viewed.json').write_text(json.dumps(rules, indent=2))
print('Viewed fixture ready: three review files and one trusted receipt.')
