#!/usr/bin/env python3
"""Add a scene spread to a storyboard document and fill it by object.label.

  scene_fill.py DOC.designcraft SCENE.json OUT.designcraft [--source-spread 3]

SCENE.json (all keys optional; text keys take strings, image keys take file paths):
  text:   running_title running_sub badge section hand_note scene_id title cue time purpose
          staging_1..3 revision overall_design location_ref character_shared character_lead
          asset_geometry match_note exact_prompt strip_note source_image_caption result_image_caption
          page_no_left page_no_right
  images: hero_image source_image result_image sketch_left sketch_right
  refs:   ["a.png", "b.png", ...]  contact strip, 1..12 images; layout adapts to the count
It duplicates the source spread at the end of the document, so the template spread stays reusable.
"""
import json, os, subprocess, sys

CLI = "/home/jesse/.local/bin/designcraft-cli"
HERE = os.path.dirname(os.path.abspath(__file__))
BUILD = json.load(open(os.path.join(HERE, "studio-template-build.json")))
LAYOUTS = BUILD["contact_strip"]["layouts"]


def run(script, doc, save=None):
    path = "/tmp/_scene_fill.dcs"
    json.dump(script, open(path, "w"))
    cmd = [CLI, "script", path, "--in", doc]
    if save:
        cmd += ["--save", save]
    out = subprocess.run(cmd, capture_output=True, text=True)
    if out.returncode:
        sys.exit(out.stderr or out.stdout)
    return json.loads(out.stdout)["results"]


def c(name, **p):
    return {"command": name, "params": p}


def main():
    doc, scene_json, out = sys.argv[1:4]
    src = int(sys.argv[sys.argv.index("--source-spread") + 1]) if "--source-spread" in sys.argv else BUILD["scene_spread_index"]
    sc = json.load(open(scene_json))
    text_keys = [k for k in sc if k not in ("refs",) and not k.endswith("_path") and k not in IMAGE_KEYS]
    refs = sc.get("refs", [])
    labels = sorted({l["label"] for l in BUILD["labels"] if l["spread"] == src})
    n_holes = sum(1 for l in BUILD["labels"] if l["spread"] == src and l["label"] == "strip_hole")
    # phase 1: duplicate the spread, then find the labelled frames (the new ids are the highest)
    steps = [c("layout.pages.duplicateSpread", spread=src, atEnd=True)]
    steps += [c("object.findByLabel", label=l) for l in labels]
    res = run(steps, doc, save=out)
    new = {}
    for l, r in zip(labels, res[1:]):
        ids = r["ids"]
        spread_count = sum(1 for _ in ids)
        new[l] = ids[-1]          # duplicates are created last, so the newest id is on the new spread
        if l == "strip_hole":
            holes = ids[-n_holes:]
    page = res[0]["page"]
    # phase 2: story ids for text frames
    texts = {k: v for k, v in sc.items() if k in new and k not in IMAGE_KEYS and k != "refs"}
    res = run([c("story.get", frame=new[k]) for k in texts], out)
    story = {k: r["story"] if "story" in r else r.get("id") for k, r in zip(texts, res)}
    steps = [c("story.setText", story=story[k], text=v) for k, v in texts.items()]
    # images
    for k in IMAGE_KEYS:
        if k in sc and k in new:
            steps += [c("place.load", path=os.path.abspath(sc[k])),
                      c("place.drop", x=0, y=0, frame=new[k]),
                      c("object.fit", mode="fillProportionally", ids=[new[k]])]
            if k + "_hint" in new:
                steps.append(c("object.setFlags", ids=[new[k + "_hint"]], hidden=True))
    # adaptive contact strip
    if refs:
        n = len(refs)
        lay = LAYOUTS[str(n)]
        steps.append(c("transform.set", x=lay["band"][0], y=lay["band"][1], width=lay["band"][2] - lay["band"][0],
                       height=lay["band"][3] - lay["band"][1], ids=[new["contact_strip"]]))
        steps += [c("object.setFlags", ids=[h], hidden=(n > 6)) for h in holes]
        for k in range(1, 13):
            lab = "ref_%d" % k
            if k <= n:
                r = lay["slots"][k - 1]
                steps += [c("object.setFlags", ids=[new[lab]], hidden=False),
                          c("transform.set", x=r[0], y=r[1], width=r[2] - r[0], height=r[3] - r[1], ids=[new[lab]]),
                          c("place.load", path=os.path.abspath(refs[k - 1])),
                          c("place.drop", x=0, y=0, frame=new[lab]),
                          c("object.fit", mode="fillProportionally", ids=[new[lab]])]
            elif lab in new:
                steps.append(c("object.setFlags", ids=[new[lab]], hidden=True))
    run(steps, out, save=out)
    print("scene spread added at page", page + 1, "->", out)


IMAGE_KEYS = ["hero_image", "source_image", "result_image", "sketch_left", "sketch_right"]

if __name__ == "__main__":
    main()
