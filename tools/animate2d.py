#!/usr/bin/env python3
"""animate2d: any cut-out PNG -> puppet-animated video, via effectcraft-cli.

Usage:
  animate2d.py IMAGE OUT.mp4 [--motion sway|bounce|wave|idle] [--seconds 4]
               [--fps 24] [--size 1080x1080] [--amount 1.0] [--bg #000000]
               [--format h264|webm|prores|gif|png] [--keep-project]

Pins are placed from the image's alpha bounding box: two feet pinned at the
bottom, a starch pin through the body, and a recorded "leader" pin near the
top whose motion drives the animation (with follow-through on a tip pin).
For --format webm/prores the background is transparent (rgba).
"""
import argparse, json, math, subprocess, sys, tempfile
from pathlib import Path

from PIL import Image


def alpha_bbox(path):
    im = Image.open(path).convert("RGBA")
    box = im.getchannel("A").point(lambda a: 255 if a > 16 else 0).getbbox()
    box = box or (0, 0, *im.size)
    a = im.getchannel("A").load()
    tip = next(([x, y + 2] for y in range(box[1], box[3]) for x in range(box[0], box[2]) if a[x, y] > 128), None)
    return im.size, box, tip


def leader_samples(motion, seconds, w, h, amount, step=1 / 12):
    """[[t, dx, dy], ...] displacement of the leader pin over time."""
    out, n = [], int(seconds / step) + 1
    for i in range(n):
        t = i * step
        p = 2 * math.pi * t
        if motion == "sway":
            dx, dy = 0.10 * w * math.sin(p / 2), 0.01 * h * math.sin(p)
        elif motion == "bounce":
            dx, dy = 0.0, -0.06 * h * abs(math.sin(p))
        elif motion == "wave":
            dx, dy = 0.12 * w * math.sin(p), 0.03 * h * math.sin(2 * p)
        else:  # idle / breathe
            dx, dy = 0.02 * w * math.sin(p / 3), -0.025 * h * (0.5 - 0.5 * math.cos(p / 2))
        out.append([round(t, 4), dx * amount, dy * amount])
    return out


def build_commands(a, image, size, box, top):
    (cw, ch) = map(int, a.size.split("x"))
    x0, y0, x1, y1 = box
    bw, bh = x1 - x0, y1 - y0
    cx = (x0 + x1) / 2
    leader = [cx, y0 + 0.25 * bh]
    tip = top or [cx, y0 + 0.05 * bh]
    scale = min(cw * 0.8 / bw, ch * 0.85 / bh) * 100
    samples = [[t, leader[0] + dx, leader[1] + dy]
               for t, dx, dy in leader_samples(a.motion, a.seconds, bw, bh, a.amount)]
    L = "Character"
    pin = lambda kind, pos, **kw: {"command": "puppet.addPin", "params": {
        "layer": L, "kind": kind, "position": pos, "time": 0, **kw}}
    return [
        {"command": "comp.new", "params": {"name": "animate2d", "width": cw, "height": ch,
                                           "frameRate": a.fps, "duration": a.seconds,
                                           "background": a.bg, "open": True}},
        {"command": "file.import", "params": {"paths": [str(image)]}},
        {"command": "layer.addItem", "params": {"item": image.name, "position": [cw / 2, ch * 0.52]}},
        {"command": "layer.rename", "params": {"layer": 1, "name": L}},
        pin("position", [x0 + 0.3 * bw, y1 - 0.03 * bh], triangles=300, expansion=3),
        pin("position", [x0 + 0.7 * bw, y1 - 0.03 * bh]),
        pin("starch", [cx, y0 + 0.6 * bh]),
        pin("advanced", leader),
        pin("position", tip),
        {"command": "prop.set", "params": {"layer": L, "path": "transform/scale",
                                           "value": [scale, scale, 100]}},
    ], samples


def motion_commands(uids, samples, L="Character"):
    leader, tip = uids[3], uids[4]
    return [
        {"command": "puppet.recordPin", "params": {"layer": L, "pin": leader, "samples": samples,
                                                   "start": 0, "smoothing": 10}},
        {"command": "puppet.follow", "params": {"layer": L, "leader": leader, "pins": [tip],
                                                "delay": 0.12, "amount": 70}},
    ]


def find_pins(node):
    if isinstance(node, dict):
        if isinstance(node.get("pins"), list) and node["pins"] and "pin" in node["pins"][0]:
            return node["pins"]
        node = list(node.values())
    if isinstance(node, list):
        for v in node:
            r = find_pins(v)
            if r:
                return r
    return []


def sh(*cmd):
    r = subprocess.run(cmd, text=True, capture_output=True)
    if r.returncode:
        sys.exit(f"FAILED: {' '.join(cmd)}\n{r.stdout[-3000:]}\n{r.stderr[-3000:]}")
    return r.stdout


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("image", type=Path)
    ap.add_argument("out", type=Path)
    ap.add_argument("--motion", default="sway", choices=["sway", "bounce", "wave", "idle"])
    ap.add_argument("--seconds", type=float, default=4)
    ap.add_argument("--fps", type=float, default=24)
    ap.add_argument("--size", default="1080x1080")
    ap.add_argument("--amount", type=float, default=1.0)
    ap.add_argument("--bg", default="#000000")
    ap.add_argument("--format", default="h264")
    ap.add_argument("--keep-project", action="store_true")
    a = ap.parse_args()

    image = a.image.resolve()
    size, box, top = alpha_bbox(image)
    cmds = build_commands(a, image, size, box, top)
    with tempfile.TemporaryDirectory() as td:
        proj = (a.out.with_suffix(".ecproj") if a.keep_project else Path(td) / "a.ecproj").resolve()
        setup, samples = cmds
        cf = Path(td) / "setup.json"
        cf.write_text(json.dumps(setup))
        sh("effectcraft-cli", "--empty", "run", "--commands", str(cf), "--save-as", str(proj))
        info = json.loads(sh("effectcraft-cli", "--project", str(proj), "--json", "exec",
                             "puppet.info", "--params", json.dumps({"layer": "Character"})))
        uids = [p["pin"] for p in find_pins(info)]
        if len(uids) < 5:
            sys.exit(f"expected 5 puppet pins, got {len(uids)} (pin landed off the artwork?)")
        cf.write_text(json.dumps(motion_commands(uids, samples)))
        sh("effectcraft-cli", "--project", str(proj), "run", "--commands", str(cf), "--save")
        extra = ["--channels", "rgba"] if a.format in ("webm", "prores", "png") else []
        sh("effectcraft-cli", "render", "--project", str(proj), "--out", str(a.out.resolve()),
           "--format", a.format, *extra)
    print(a.out.resolve())


if __name__ == "__main__":
    main()
