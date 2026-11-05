#!/usr/bin/env python3
"""split_turnaround.py SHEET.png OUTDIR  -> front, three-quarter-left, right, three-quarter-right, back (transparent sheets)"""
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert('RGBA'); W, H = im.size; px = im.getchannel('A').load()
cols = [any(px[x, y] > 20 for y in range(0, H, 3)) for x in range(W)]
runs, s = [], None
for x, c in enumerate(cols + [False]):
    if c and s is None: s = x
    if not c and s is not None:
        if x - s > 60: runs.append((s, x))
        s = None
for name, (x0, x1) in zip(['front', 'three-quarter-left', 'right', 'three-quarter-right', 'back'], runs):
    c = im.crop((max(0, x0 - 10), 0, min(W, x1 + 10), H)); c = c.crop(c.getbbox())
    out = Image.new('RGBA', (c.width + 60, c.height + 60), (0, 0, 0, 0)); out.paste(c, (30, 30))
    out.save(f'{sys.argv[2]}/{name}.png')
print(len(runs), 'views')
