#!/usr/bin/env python3
"""split_white_turnaround.py SHEET.png OUTDIR -> front, three-quarter-left, profile, three-quarter-right, back (white bg)"""
import sys, os
from PIL import Image
im = Image.open(sys.argv[1]).convert('RGB'); W, H = im.size; px = im.load()
cols = [sum(1 for y in range(0, H, 2) if sum(px[x, y]) < 690) > 2 for x in range(W)]
runs, s = [], None
for x, c in enumerate(cols + [False]):
    if c and s is None: s = x
    if not c and s is not None:
        if x - s > 30: runs.append((s, x))
        s = None
os.makedirs(sys.argv[2], exist_ok=True)
if len(runs) != 5:
    runs = [(int(i * W / 5), int((i + 1) * W / 5)) for i in range(5)]
names = ['front', 'three-quarter-left', 'profile', 'three-quarter-right', 'back']
for name, (x0, x1) in zip(names, runs):
    pad = 0 if len(runs)==5 and runs[0][0]==0 else 20; c = im.crop((max(0, x0 - pad), 0, min(W, x1 + pad), H))
    cp = c.load(); cw, ch = c.size
    has = [sum(1 for y in range(0, ch, 2) if sum(cp[x, y]) < 690) > 2 for x in range(cw)]
    best, cur, b0 = (0, 0), None, 0
    for x, h in enumerate(has + [False]):
        if h and cur is None: cur = x
        if not h and cur is not None:
            if x - cur > best[1] - best[0]: best = (cur, x)
            cur = None
    from PIL import ImageDraw
    d = ImageDraw.Draw(c)
    if best[0] > 0: d.rectangle((0, 0, best[0] - 1, ch), fill='white')
    if best[1] < cw: d.rectangle((best[1], 0, cw, ch), fill='white')
    c.save(f'{sys.argv[2]}/{name}.png')
print(len(runs), 'views', runs)
