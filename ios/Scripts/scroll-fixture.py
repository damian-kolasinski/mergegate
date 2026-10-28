#!/usr/bin/env python3
"""Submit a long, highlighted diff with wide lines to a disposable smoke hub for scrolling checks."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--pack', type=Path, help='Installed Swift pack directory; omit for plain-text fallback')
parser.add_argument('--functions', type=int, default=30, help='Changed functions, the diff grows by about nine lines each')
args = parser.parse_args()
root = args.root.resolve()
json.loads((root / 'listening.json').read_text())
repo = root / 'repo'
env = dict(os.environ, MERGEGATE_HOME=str(root / 'hub'))
mg = os.environ.get('MG_BIN') or shutil.which('mg')

def run(*args):
    result = subprocess.run(args, cwd=repo, env=env, text=True, capture_output=True)
    if result.returncode:
        raise SystemExit(result.stdout + result.stderr)
    return result.stdout.strip()

if args.pack:
    pack = args.pack.resolve()
    if not (pack / 'pack.json').is_file() or not (pack / 'swift.wasm').is_file():
        raise SystemExit('Pass an installed Swift pack directory containing pack.json and swift.wasm.')
    archive = root / 'swift.tar.gz'
    with tarfile.open(archive, 'w:gz') as target:
        for entry in sorted(pack.iterdir()):
            if entry.is_file() and not entry.name.startswith('.'):
                target.add(entry, arcname=entry.name)
    run(mg, 'lang', 'add', '--from', str(archive))

def source(revised):
    lines = ['import Foundation', '']
    for number in range(1, args.functions + 1):
        rate = f'rate{number}'
        lines += [
            f'func deliveryFee{number}(for order: Order, in region: Region) -> Decimal {{',
            f'    let {rate} = region.publishedRates[{number}] ?? .zero',
            f'    let threshold = order.subtotal > region.freeDeliveryThreshold ? Decimal.zero : {rate}'
            if revised else f'    let threshold = {rate}',
            '    // Delivery is charged once per checkout, after discounts and before taxes, using the rate the region published at the time the order was placed; refunds reuse the same rate.'
            if revised and number % 2 else '    // Delivery is charged once per checkout.',
            f'    return threshold * order.weightMultiplier(region: region, fallback: .standard, rounding: .bankers, scale: {number % 7 + 2})',
            '}',
        ]
    return '\n'.join(lines) + '\n'

run('git', 'switch', 'main')
(repo / 'Delivery.swift').write_text(source(False))
run('git', 'add', '.')
run('git', 'commit', '-m', 'Add delivery fees')
run('git', 'switch', '-c', 'scroll-long-diff')
(repo / 'Delivery.swift').write_text(source(True))
run('git', 'add', '.')
run('git', 'commit', '-m', 'Waive delivery above the free threshold')
guide = root / 'scroll-guide.md'
guide.write_text('''Waive delivery above each region's free-delivery threshold.

## Files

- `Delivery.swift` — Every fee applies the threshold.
''')
submitted = json.loads(run(mg, '--json', 'submit', '--title', 'Waive delivery above the free threshold',
                           '--guide', str(guide)))
(root / 'scroll.json').write_text(json.dumps(submitted, indent=2))
print(json.dumps(submitted, indent=2))
