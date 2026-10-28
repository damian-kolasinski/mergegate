#!/usr/bin/env python3
"""Attach a portrait image and generated video to a disposable review fixture (requires ffmpeg)."""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import zlib

root = Path(sys.argv[1]).resolve()
json.loads((root / 'listening.json').read_text())
ffmpeg = shutil.which('ffmpeg')
if not ffmpeg:
    raise SystemExit('ffmpeg is required only to generate the disposable demo test clip.')
subprocess.run([sys.executable, str(Path(__file__).with_name('review-fixture.py')), str(root)], check=True)

def chunk(name, data):
    return struct.pack('!I', len(data)) + name + data + struct.pack('!I', zlib.crc32(name + data))

width, height = 900, 1200
pixels = b''.join(b'\0' + b''.join(bytes((x * 255 // width, y * 255 // height, 128))
                                 for x in range(width)) for y in range(height))
image = root / 'portrait.png'
image.write_bytes(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('!2I5B', width, height, 8, 2, 0, 0, 0))
                  + chunk(b'IDAT', zlib.compress(pixels)) + chunk(b'IEND', b''))
video = root / 'demo.mp4'
subprocess.run([ffmpeg, '-hide_banner', '-loglevel', 'error', '-f', 'lavfi', '-i',
                'testsrc2=size=640x360:rate=15', '-t', '20', '-c:v', 'libx264', '-pix_fmt',
                'yuv420p', str(video)], check=True)
mg = os.environ.get('MG_BIN') or shutil.which('mg')
subprocess.run([mg, 'attach', 'mobile-smoke#2', str(image), str(video)], cwd=root / 'repo',
               env=dict(os.environ, MERGEGATE_HOME=str(root / 'hub')), check=True)
