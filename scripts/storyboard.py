#!/usr/bin/env python3
"""Use DeckCraft for every document operation. Page and transcript line numbers start at 1."""
import argparse
import json
import os
import shlex
import uuid
from pathlib import Path
import subprocess
import tempfile

CLI = "deckcraft-cli"
TEMPLATE = Path(__file__).resolve().parents[1] / "references/storyboard/Storyboard.potx"


def deck(path, commands, save=None):
    args = [CLI, "run", "--in", str(path)]
    for name, params in commands:
        args += ["--cmd", name + "=" + json.dumps(params)]
    if save:
        subprocess.run(args + ["--save", str(save)], check=True, stdout=subprocess.DEVNULL)
        return
    out = subprocess.check_output(args + ["--print"], text=True)
    return [json.loads(line.partition(" → ")[2]) for line in out.splitlines()]


def shapes(file, page):
    return deck(file, [("slide.inspect", {"index": page - 1})])[0]["shapes"]


def contents(file):
    info = json.loads(subprocess.check_output([CLI, "info", str(file)]))
    ops = [("slide.inspect", {"index": slide["index"]}) for slide in info["slides"]]
    return [{s["name"]: s.get("text", "") for s in page["shapes"]} for page in deck(file, ops)]


def fields(file, page, values, images, root):
    slots = {s["name"]: s for s in shapes(file, page)}
    ops = [("slide.go", {"index": page - 1})]
    for name, text in values.items():
        if name not in slots or slots[name]["kind"] != "shape":
            raise ValueError(f"Page {page} has no text field {name!r}; use read to list fields.")
        ops.append(("text.set", {"id": slots[name]["id"], "text": text}))
    for name, src in images.items():
        ops.append(("insert.picture", {"name": name + " artwork", "rect": slots[name]["box"],
                                       "path": str((root / src).resolve(strict=True))}))
    return ops


def build(plan, stage, result):
    data = json.loads(plan.read_text())
    root = plan.parent
    (root / data["audio"]).resolve(strict=True)
    transcript = json.loads((root / data["transcript"]).read_text())["paragraphs"]
    if not transcript:
        raise ValueError("No numbered transcript lines; no storyboard created.")
    scenes = data["scenes"]
    if not scenes:
        raise ValueError("Choose a scene plan before creating a storyboard.")
    covered = []
    # Reuse the blank scene page; keep the overview, sequence map and final handoff intact.
    deck(TEMPLATE, [("slide.duplicate", {"index": 2}) for _ in scenes[1:]], stage)
    ops = []
    for page, key in [(1, "overview"), (2, "sequence"), (3 + len(scenes), "handoff")]:
        card = data[key]
        ops += fields(stage, page, card["fields"], card.get("images", {}), root)
    for i, scene in enumerate(scenes):
        values = dict(scene["fields"])
        if "lines" in scene:
            first, last = scene["lines"]
            if not 1 <= first <= last <= len(transcript):
                raise ValueError(f"Scene {i + 1}: invalid transcript range")
            lines = transcript[first - 1:last]
            covered.extend(range(first, last + 1))
            values["cue"] = "\n".join(item["text"] for item in lines)
            values.setdefault("time", f'{lines[0]["start"]:.2f}–{lines[-1]["end"]:.2f} s')
        values["scene_id"] = f"S{i + 1:02}"
        ops += fields(stage, i + 3, values, scene.get("images", {}), root)
    if covered != list(range(1, len(transcript) + 1)):
        raise ValueError("Scene plan must cover every transcript line once, in order.")
    deck(stage, ops, result)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="action", required=True)
    for action in ("create", "list", "read", "edit", "add", "delete", "layer"):
        cmd = sub.add_parser(action)
        cmd.add_argument("document", type=Path)
        if action == "create":
            cmd.add_argument("plan", type=Path)
        if action not in ("create", "list"):
            cmd.add_argument("page", help="Page number or scene ID, e.g. S02; add inserts after it")
        if action in ("edit", "layer"):
            cmd.add_argument("field")
            cmd.add_argument("value", help="New text, or an image path for layer")
    a = p.parse_args()
    path = a.document.resolve()
    if path.suffix != ".deckcraft":
        p.error("Use a .deckcraft project; the template stays read-only.")
    if a.action == "list":
        for page, card in enumerate(contents(path), 1):
            print(f'{page}: {card.get("scene_id", "")} {card.get("title", "")}')
        return
    selector = getattr(a, "page", "1")
    if selector.isdecimal():
        page = int(selector)
    else:
        hits = [i for i, card in enumerate(contents(path), 1) if card.get("scene_id") == selector]
        if len(hits) != 1:
            p.error(f"Expected one scene named {selector!r}; found {len(hits)}.")
        page = hits[0]
    if page < 1:
        p.error("Pages begin at 1.")
    if a.action == "read":
        print(json.dumps({s["name"]: s.get("text", s["kind"]) for s in shapes(path, page)},
                         indent=2, ensure_ascii=False))
        return
    if a.action == "create" and path.exists():
        p.error(f"Already exists: {path}")
    before = None if a.action == "create" else path.stat()
    with tempfile.TemporaryDirectory(dir=path.parent) as tmp:
        result = Path(tmp) / "saved.deckcraft"
        if a.action == "create":
            build(a.plan.resolve(), Path(tmp) / "layout.deckcraft", result)
        elif a.action in ("edit", "layer"):
            texts = {a.field: a.value} if a.action == "edit" else {}
            images = {a.field: a.value} if a.action == "layer" else {}
            deck(path, fields(path, page, texts, images, Path.cwd()), result)
        else:
            if "scene_id" not in {s["name"] for s in shapes(path, page)}:
                p.error("Select a scene page; overview, sequence and handoff pages are protected.")
            if a.action == "delete":
                deck(path, [("slide.delete", {"index": page - 1})], result)
                page = max(1, page - 1)
            else:
                stage = Path(tmp) / "added.deckcraft"
                deck(path, [("slide.reuse", {"path": str(TEMPLATE), "slides": [2],
                                            "keepFormatting": True, "at": page})], stage)
                page += 1
                new_id = "S-" + uuid.uuid4().hex[:8]
                deck(stage, fields(stage, page, {"scene_id": new_id, "title": "New scene"},
                                   {}, Path.cwd()), result)
        if before is None:
            os.link(result, path)  # Never overwrite a project created by another caller.
        elif (before.st_ino, before.st_mtime_ns) == (path.stat().st_ino, path.stat().st_mtime_ns):
            os.replace(result, path)
        else:
            p.error("The deck changed on disk during the edit; reopen it and retry.")
    print(f"Saved: {path}\nReview: {shlex.join([__file__, 'read', str(path), str(page)])}")


if __name__ == "__main__":
    main()
