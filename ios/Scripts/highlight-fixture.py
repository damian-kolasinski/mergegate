#!/usr/bin/env python3
"""Seed real source and package an installed Swift grammar for late-install UI verification."""
import argparse
from pathlib import Path
import subprocess
import sys
import tarfile

parser = argparse.ArgumentParser()
parser.add_argument('root', type=Path)
parser.add_argument('--pack', required=True, type=Path, help='Installed Swift pack directory containing pack.json and swift.wasm')
args = parser.parse_args()
pack = args.pack.resolve()
if not (pack / 'pack.json').is_file() or not (pack / 'swift.wasm').is_file():
    raise SystemExit('Pass an installed Swift pack directory.')
subprocess.run([sys.executable, str(Path(__file__).with_name('source-fixture.py')), str(args.root)], check=True)
archive = args.root.resolve() / 'swift.tar.gz'
with tarfile.open(archive, 'w:gz') as target:
    for entry in sorted(pack.iterdir()):
        if entry.is_file() and not entry.name.startswith('.'):
            target.add(entry, arcname=entry.name)
print(f'MG_IOS_TEST_HIGHLIGHT={archive}')
