"""Stage 1: Cut pose/view/part sheets via photocraft-cli and QA-review frames."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import json
import math
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageOps
from scipy import ndimage as ndi

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)
from pipeline.step_cutter_review import (
    PARTS,
    RUNTIME,
    TEMPLATE,
    background_candidates as _background_candidates,
    detect_components as _components,
    frame_review as _frame_review,
    recut_flagged_sheet,
    save_parts as _save_parts,
    write_vector_limb_masks_svg as _write_vector_limb_masks_svg,
)

VAULT_LAYOUTS = (
    Path(__file__).resolve().parents[1]
    / "exports"
    / "templates"
    / "vault_sheet_layouts.json"
)
ACTIONS = [
    "standing",
    "walking",
    "running",
    "jumping",
    "dancing",
    "waving",
    "clapping",
    "looking_up",
    "looking_down",
    "arms_wide",
]
VIEWS = ["front", "quarter_front", "side", "opposite_quarter", "back"]
ALIASES = {
    "standing": ["standing", "stand"],
    "walking": ["walking", "walk"],
    "running": ["running", "run"],
    "jumping": ["jumping", "jump"],
    "dancing": ["dancing", "dance"],
    "waving": ["waving", "wave"],
    "clapping": ["clapping", "clap"],
    "looking_up": ["looking upward", "looking upwards", "looking up"],
    "looking_down": ["looking downward", "looking downwards", "looking down"],
    "arms_wide": [
        "arms open wide",
        "arms wide open",
        "arms wide",
        "arms out",
        "arms spread",
    ],
    "pointing": ["pointing", "point"],
    "sitting": ["sitting", "sit"],
    "winking": ["winking", "wink"],
}


def exploded_prompt(character: str, description: str = "") -> str:
    return json.loads(TEMPLATE.read_text())["promptTemplate"].format(
        character=character, description=description or "use supplied reference"
    )


def _find_image_cli() -> str | None:
    cand = os.environ.get("PHOTOCRAFT_CLI") or shutil.which("photocraft-cli")
    if not cand:
        return None
    try:
        proc = subprocess.run(
            [cand, "commands", "--json", "--filter", "layer.removeBackground"],
            capture_output=True, text=True, timeout=5,
        )
    except OSError:
        return None
    try:
        commands = json.loads(proc.stdout)
    except ValueError:
        commands = None
    supported = isinstance(commands, list) and any(
        isinstance(command, dict) and command.get("id") == "layer.removeBackground"
        for command in commands
    )
    if proc.returncode or not supported:
        detail = (proc.stdout or proc.stderr).strip() or f"exit {proc.returncode}"
        raise RuntimeError(
            f"PhotoCraft CLI {cand} does not expose layer.removeBackground: {detail}. "
            "Set PHOTOCRAFT_CLI to an installed CLI that provides that command."
        )
    return cand


def _find_vector_cli() -> str | None:
    return os.environ.get("VECTORCRAFT_CLI") or shutil.which("vectorcraft-cli")


def _ocr_labels(source: Path) -> list[dict]:
    if not shutil.which("tesseract"):
        return []
    proc = subprocess.run(
        ["tesseract", str(source), "stdout", "--psm", "11", "tsv"],
        capture_output=True,
        text=True,
        timeout=45,
        check=True,
    )
    lines: dict[tuple, list] = {}
    for word in csv.DictReader(io.StringIO(proc.stdout), delimiter="\t"):
        if not word.get("text", "").strip() or float(word.get("conf", "-1")) < 15:
            continue
        key = tuple(word[k] for k in ("page_num", "block_num", "par_num", "line_num"))
        lines.setdefault(key, []).append(word)
    found = []
    for words in lines.values():
        text = " ".join(w["text"] for w in words).lower()
        normalized = re.sub("[^a-z ]", " ", text)
        for label, aliases in ALIASES.items():
            if any(
                re.search(r"\b" + re.escape(a) + r"\b", normalized) for a in aliases
            ):
                left = min(int(w["left"]) for w in words)
                right = max(int(w["left"]) + int(w["width"]) for w in words)
                top = min(int(w["top"]) for w in words)
                found.append(
                    {
                        "label": label,
                        "x": (left + right) / 2,
                        "y": top,
                        "text": text,
                        "confidence": min(float(w["conf"]) for w in words) / 100,
                        "engine": "tesseract",
                    }
                )
                break
    return found


def _validate_box(box, size):
    if not isinstance(box, (list, tuple)) or len(box) != 4:
        raise ValueError("box must be [left, top, right, bottom] in source pixels")
    if any(not isinstance(v, (int, float)) or not math.isfinite(v) for v in box):
        raise ValueError("box coordinates must be finite numbers")
    x0, y0, x1, y1 = map(int, box)
    if not (0 <= x0 < x1 <= size[0] and 0 <= y0 < y1 <= size[1]):
        raise ValueError(f"box outside source: {box}")
    return x0, y0, x1, y1


def _clean_crop(source: Image.Image, box, pc_bin: str) -> Image.Image:
    x0, y0, x1, y1 = box
    pad = 8
    px0, py0 = max(0, x0 - pad), max(0, y0 - pad)
    px1, py1 = min(source.width, x1 + pad), min(source.height, y1 + pad)
    rect = [x0 - px0, y0 - py0, x1 - x0, y1 - y0]
    padded = source.crop((px0, py0, px1, py1)).convert("RGBA")

    with tempfile.TemporaryDirectory() as tmpd:
        in_p, out_p = Path(tmpd) / "in.png", Path(tmpd) / "out.png"
        padded.save(in_p)
        proc = subprocess.run(
            [
                pc_bin,
                "run",
                str(in_p),
                "--cmd",
                "layer.removeBackground",
                "--params",
                json.dumps({"refine": True}),
                "--out",
                str(out_p),
            ],
            capture_output=True,
            text=True,
            timeout=120,
        )
        if proc.returncode or not out_p.is_file():
            raise RuntimeError(
                f"photocraft-cli layer.removeBackground failed for frame {box}: "
                + ((proc.stderr or proc.stdout).strip().splitlines() or ["no output"])[
                    -1
                ]
            )
        with Image.open(out_p) as out:
            cut = out.convert("RGBA").crop(
                (rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3])
            )

    cut_arr = np.array(cut)
    cut_arr[cut_arr[:, :, 3] < 48, 3] = 0
    cut = Image.fromarray(cut_arr)
    if cut.getchannel("A").getbbox() is None:
        raise ValueError(f"PhotoCraft selection found no figure in frame {box}")
    return cut


def _panel_grid(image, layout):
    """Measure the guide's blue dividers and captions, excluding them from artwork cuts."""
    grid = layout.get("grid")
    if not grid:
        return layout
    pixels = np.asarray(image).astype(np.int16)
    ink = (pixels[:, :, 2] > pixels[:, :, 0] + 12) & (pixels[:, :, 1] > pixels[:, :, 0] + 5) & (pixels[:, :, 2] < 200)
    def bands(values):
        indices = np.flatnonzero(values)
        groups = np.split(indices, np.flatnonzero(np.diff(indices) > 1) + 1)
        return [(int(g[0]), int(g[-1])) for g in groups if len(g)]
    horizontal = bands(ink.mean(axis=1) > .7)
    vertical = bands(ink.mean(axis=0) > .65)
    rows, columns = int(grid["rows"]), int(grid["columns"])
    if len(horizontal) != rows + 1 or len(vertical) != columns + 1:
        raise ValueError(f"Expected {columns+1} vertical and {rows+1} horizontal panel dividers; found {len(vertical)}, {len(horizontal)}. Correct the layout before cutting.")
    frames = []
    for index, frame in enumerate(layout["frames"]):
        row, col = int(frame.get("gridRow",index//columns)),int(frame.get("gridColumn",index%columns))
        top, bottom = horizontal[row][1] + 2, horizontal[row+1][0] - 2
        left, right = vertical[col][1] + 2, vertical[col+1][0] - 2
        # A common caption cutoff across the row keeps registration consistent.
        heading_bottom = top
        for c in range(columns):
            start, end = vertical[c][1] + 4, vertical[c+1][0] - 4
            caption = ink[top:top + max(1, int((bottom-top)*.18)), start:end]
            occupied = np.flatnonzero(caption.any(axis=1))
            if len(occupied):
                heading_bottom = max(heading_bottom, top + int(occupied[-1]) + 3)
        frames.append({**frame, "box": [left, heading_bottom, right, bottom]})
    return {**layout, "frames": frames, "measuredGrid": {"horizontal": horizontal, "vertical": vertical}}


def _batch_mattes(image, definitions):
    """Reuse one real neural matting session for an explicit set of panel crops."""
    boxes = [_validate_box(item["box"], image.size) for item in definitions]
    def white_corners(box):
        pixels = np.asarray(image.crop(box))[:, :, :3]
        return all(corner.min() >= 235 and corner.mean() >= 245 for corner in (pixels[:3,:3],pixels[:3,-3:],pixels[-3:,:3],pixels[-3:,-3:]))
    if all(white_corners(box) for box in boxes):
        return _flat_mattes(image, boxes), "photocraft-magicWand-refineEdge"
    backend = os.getenv("MAGIC_MATTING_BACKEND", "auto")
    executable = shutil.which("rembg") if backend in ("auto", "rembg") else None
    if not executable:
        if backend == "rembg":
            raise RuntimeError("rembg is selected but is not installed")
        return None, "photocraft-layer.removeBackground"
    model = os.getenv("MAGIC_MATTING_MODEL", "birefnet-general-lite")
    with tempfile.TemporaryDirectory(prefix="magic-sheet-mattes-") as temporary:
        inputs, outputs = Path(temporary)/"inputs", Path(temporary)/"outputs"
        inputs.mkdir(); outputs.mkdir()
        for i, box in enumerate(boxes):
            image.crop(box).save(inputs / f"{i:03}.png")
        process = subprocess.run([executable, "p", "-m", model, "-dc", str(inputs), str(outputs)], capture_output=True, text=True, timeout=900)
        if process.returncode:
            raise RuntimeError(f"Neural panel matting failed: {(process.stderr or process.stdout)[-1200:]}")
        result = {}
        for i, box in enumerate(boxes):
            with Image.open(outputs / f"{i:03}.png") as output:
                pixels = output.convert("RGBA")
                if not pixels.getchannel("A").getbbox():
                    raise ValueError(f"No subject found in panel {i+1}")
                result[box] = pixels
        return result, f"rembg/{model}"


def _flat_mattes(image, boxes):
    """Connected background selection preserves enclosed whites and fine drawn lines."""
    from concurrent.futures import ThreadPoolExecutor
    executable = _find_image_cli()
    if not executable:
        raise RuntimeError("photocraft-cli is required for flat-background panel extraction")
    with tempfile.TemporaryDirectory(prefix="magic-flat-panels-") as temporary:
        directory = Path(temporary)
        def matte(item):
            index, box = item
            source, output = directory/f"{index}-source.png", directory/f"{index}.png"
            image.crop(box).save(source)
            args = [executable,"run",str(source),"--cmd","layer.new.layerFromBackground","--params",'{"ifPresent":true}',
                "--cmd","select.magicWand","--params",'{"x":0,"y":0,"tolerance":18,"contiguous":true,"antiAlias":true}',
                "--cmd","select.inverse","--cmd","select.refineEdge","--params",
                '{"radius":1,"feather":0.2,"contrast":20,"shiftEdge":0,"decontaminate":false,"output":"layerMask"}',"--out",str(output)]
            process = subprocess.run(args,capture_output=True,text=True,timeout=120)
            if process.returncode:
                raise RuntimeError(f"PhotoCraft panel {index+1} failed: {(process.stderr or process.stdout)[-800:]}")
            with Image.open(output) as result:
                rgba=result.convert("RGBA")
                if not rgba.getchannel("A").getbbox():
                    raise ValueError(f"No visible artwork in panel {index+1}")
                return box,rgba
        with ThreadPoolExecutor(max_workers=4) as executor:
            return dict(executor.map(matte,enumerate(boxes)))


def _resolve_sheet_kind(source, kind):
    if kind == "auto":
        if source.name.startswith("Create_pose_sheet"):
            kind = "action"
        elif "turnaround" in source.name.lower():
            kind = "turnaround"
        else:
            kind = "single"
    if kind not in ("action", "turnaround", "single", "exploded"):
        raise ValueError(f"Unknown sheet kind: {kind}")
    return kind


def _resolve_sheet_layout(source, image, layout):
    if isinstance(layout, (str, Path)):
        layout = json.loads(Path(layout).read_text())
    if layout is None and VAULT_LAYOUTS.exists():
        preset = json.loads(VAULT_LAYOUTS.read_text()).get("sheets", {}).get(source.name)
        if preset and hashlib.sha256(source.read_bytes()).hexdigest() == preset["sha256"]:
            layout = preset
    return _panel_grid(image, layout) if layout else layout


def _foreground_mask(image, kind, tolerance):
    raw = np.array(image)
    if raw[:, :, 3].min() >= 255:
        return ndi.binary_opening(
            ~_background_candidates(raw[:, :, :3], tolerance), iterations=2
        )
    mask = raw[:, :, 3] > 16
    probe_kind = "action" if kind == "exploded" else kind
    if kind in ("action", "turnaround", "exploded") and (
        mask.mean() > 0.5 or not _components(mask, probe_kind)
    ):
        mask = ndi.binary_opening(
            mask & ~_background_candidates(raw[:, :, :3], tolerance),
            iterations=2,
        )
    return mask


def _box_fits_any(cell_box, boxes):
    cx0, cy0, cx1, cy1 = cell_box
    for bx0, by0, bx1, by1 in boxes:
        if (
            cx0 <= (bx0 + bx1) * 0.5 <= cx1
            and cy0 <= (by0 + by1) * 0.5 <= cy1
            and (bx1 - bx0) <= (cx1 - cx0) * 1.05
            and (by1 - by0) <= (cy1 - cy0) * 1.05
        ):
            return True
    return False


def _exploded_definitions(mask, w, h, warnings):
    spec = json.loads(TEMPLATE.read_text())
    slot_boxes = [
        {
            "label": s["id"],
            "box": [
                s["cell"][0] * w // 3,
                s["cell"][1] * h // 3,
                (s["cell"][0] + 1) * w // 3,
                (s["cell"][1] + 1) * h // 3,
            ],
        }
        for s in spec["slots"]
    ]
    boxes = _components(mask, "action")
    core_ids = {"head", "torso", "arm_L", "arm_R", "leg_L", "leg_R"}
    if len(boxes) >= 6 and all(
        _box_fits_any(sb["box"], boxes) for sb in slot_boxes if sb["label"] in core_ids
    ):
        return slot_boxes
    if not boxes:
        raise ValueError("No foreground parts detected; supply --layout")
    warnings.append(
        "Exploded sheet does not match the 3x3 biped template; extracted connected components as unlabelled parts (part_00..). Supply --layout to name parts explicitly."
    )
    return [
        {
            "box": [max(0, b[0] - 3), max(0, b[1] - 3), min(w, b[2] + 3), min(h, b[3] + 3)],
            "label": f"part_{i:02d}",
            "labelSource": "unlabelled",
        }
        for i, b in enumerate(boxes)
    ]


def _match_ocr_label(b, h, labels):
    cx = (b[0] + b[2]) / 2
    max_dx = (b[2] - b[0]) * 0.65
    candidates = [
        l for l in labels
        if abs(l["x"] - cx) < max_dx and b[3] - 8 <= l["y"] <= b[3] + h * 0.11
    ]
    return min(
        candidates,
        key=lambda l: abs(l["y"] - b[3]) + abs(l["x"] - cx),
        default=None,
    )


def _try_ocr_labels(source, kind, use_ocr, warnings):
    if not (use_ocr and kind == "action"):
        return [], None
    try:
        labels = _ocr_labels(source)
    except (subprocess.SubprocessError, OSError) as exc:
        warnings.append(
            f"OCR unavailable: {type(exc).__name__}; supply labels via --layout"
        )
        labels = []
    return labels, (len(labels) or None)


def _character_box_entry(i, b, w, h, labels, is_five_turnaround):
    box = [max(0, b[0] - 3), max(0, b[1] - 3), min(w, b[2] + 3), min(h, b[3] + 3)]
    match = _match_ocr_label(b, h, labels)
    if is_five_turnaround:
        return {"box": box, "label": VIEWS[i], "labelSource": "turnaround_order_assumption", "ocr": match}
    if match:
        return {"box": box, "label": match["label"], "labelSource": "ocr", "ocr": match}
    return {"box": box, "label": f"pose_{i:02d}", "labelSource": "unlabelled", "ocr": None}


def _character_definitions(source, mask, kind, use_ocr, w, h, warnings):
    boxes = _components(mask, kind)
    if not boxes:
        raise ValueError("No foreground characters detected; supply --layout")
    labels, caption_count = _try_ocr_labels(source, kind, use_ocr, warnings)
    is_five_turnaround = kind == "turnaround" and len(boxes) == 5
    definitions = [
        _character_box_entry(i, b, w, h, labels, is_five_turnaround)
        for i, b in enumerate(boxes)
    ]
    return definitions, caption_count


def _discover_definitions(source, image, kind, layout, use_ocr, tolerance, warnings):
    definitions = (layout or {}).get("frames")
    if definitions is not None:
        return definitions, None
    w, h = image.size
    if kind == "single":
        return [{"label": "single", "box": [0, 0, w, h]}], None
    mask = _foreground_mask(image, kind, tolerance)
    if kind == "exploded":
        return _exploded_definitions(mask, w, h, warnings), None
    return _character_definitions(source, mask, kind, use_ocr, w, h, warnings)


def _expand_crop_box(image, box, crop, pc_bin):
    w, h = image.size
    for _ in range(3):
        ca = np.array(crop)[:, :, 3] > 20
        bx0, by0, bx1, by1 = box
        expanded = (
            max(0, bx0 - (6 if ca[:, :2].any() else 0)),
            max(0, by0 - (6 if ca[:2, :].any() else 0)),
            min(w, bx1 + (6 if ca[:, -2:].any() else 0)),
            min(h, by1 + (6 if ca[-2:, :].any() else 0)),
        )
        if expanded == box:
            break
        box = expanded
        crop = _clean_crop(image, box, pc_bin)
    return box, crop


def _apply_exclusion_polygons(crop, box, polygons):
    if not polygons:
        return crop
    exclusions = Image.new("L", crop.size)
    for polygon in polygons:
        if len(polygon) < 3:
            raise ValueError("excludePolygons need at least three points")
        ImageDraw.Draw(exclusions).polygon(
            [(x - box[0], y - box[1]) for x, y in polygon], fill=255
        )
    arr = np.array(crop)
    arr[np.array(exclusions) > 0, 3] = 0
    return Image.fromarray(arr)


def _extract_crops(image, definitions, kind, layout, pc_bin):
    preserve = bool((layout or {}).get("preserveRegistration"))
    batched, matting_engine = (
        _batch_mattes(image, definitions)
        if preserve
        else (None, "photocraft-layer.removeBackground")
    )
    crops, missing_parts = [], []
    for item in definitions:
        box = _validate_box(item["box"], image.size)
        try:
            crop = batched[box] if batched is not None else _clean_crop(image, box, pc_bin)
            if not item.get("parts") and not preserve:
                box, crop = _expand_crop_box(image, box, crop, pc_bin)
        except ValueError:
            if kind == "exploded" and item.get("label") in ("ears", "prop_backpack"):
                missing_parts.append(item["label"])
                continue
            raise
        crop = _apply_exclusion_polygons(crop, box, item.get("excludePolygons"))
        crops.append((item, box, crop))
    return crops, missing_parts, matting_engine


def _frame_offset(item, crop, cw, ch, padding):
    if "offset" not in item:
        return (cw - crop.width) // 2, ch - padding - crop.height
    ox, oy = item["offset"]
    valid = (
        isinstance(ox, int)
        and isinstance(oy, int)
        and 0 <= ox <= cw - crop.width
        and 0 <= oy <= ch - crop.height
    )
    if not valid:
        raise ValueError("offset would clip frame outside aligned canvas")
    return ox, oy


def _touches_boundary(crop):
    alpha = np.array(crop)[:, :, 3]
    return bool(alpha[0].any() or alpha[-1].any() or alpha[:, 0].any() or alpha[:, -1].any())


def _build_frame_entry(i, item, box, crop, ctx):
    cw, ch, padding, columns = ctx["cw"], ctx["ch"], ctx["padding"], ctx["columns"]
    output, atlas, contact, draw = ctx["output"], ctx["atlas"], ctx["contact"], ctx["draw"]
    ox, oy = _frame_offset(item, crop, cw, ch, padding)
    aligned = Image.new("RGBA", (cw, ch))
    aligned.paste(crop, (ox, oy))
    name = re.sub(r"[^a-zA-Z0-9_]+", "_", str(item.get("label", f"pose_{i:02d}")))
    file = f"frames/{i:02d}_{name}.png"
    aligned.save(output / file)
    ax, ay = i % columns * cw, i // columns * ch
    atlas.paste(aligned, (ax, ay))
    cy = i // columns * (ch + 28)
    contact.paste(aligned, (ax, cy), aligned.getchannel("A"))
    draw.text((ax + 6, cy + ch + 5), f"{i}: {name}", fill="#17181a")
    parts_def = item.get("parts") or {}
    parts = _save_parts(crop, box, (ox, oy), parts_def, output, i, TEMPLATE) if parts_def else {}
    svg_mask = (
        _write_vector_limb_masks_svg(file, (cw, ch), box, (ox, oy), parts_def, output, i)
        if parts_def
        else None
    )
    ctx["editable"]["frames"].append(
        {**item, "label": item.get("label", name), "box": list(box), "parts": parts_def}
    )
    issues = (
        _frame_review(
            crop, i, ctx["boxes"], ctx["median_area"],
            ctx["sheet_bg"][box[1] : box[3], box[0] : box[2]],
        )
        if ctx["kind"] != "exploded"
        else []
    )
    default_src = "explicit" if ctx["layout"] or ctx["kind"] == "exploded" else "unlabelled"
    return {
        "index": i,
        "label": item.get("label", name),
        "file": file,
        "holdFrames": item.get("holdFrames", 1),
        "size": [cw, ch],
        "qa": {
            "touchesCropBoundary": _touches_boundary(crop),
            "alphaReviewRequired": True,
            "issues": issues,
        },
        "sourceBox": list(box),
        "sourceOffset": [ox, oy],
        "anchor": [cw / 2, ch - padding],
        "atlasRect": [ax, ay, cw, ch],
        "uv": [ax / atlas.width, 1 - (ay + ch) / atlas.height, cw / atlas.width, ch / atlas.height],
        "labelSource": item.get("labelSource", default_src),
        "ocr": item.get("ocr"),
        "parts": parts,
        "vectorMaskSvg": svg_mask,
    }


def _export_frames_and_atlas(image, crops, kind, layout, tolerance, output):
    padding = 8
    cw = max(c.width for _, _, c in crops) + padding * 2
    ch = max(c.height for _, _, c in crops) + padding * 2
    columns = min(5, len(crops))
    rows = math.ceil(len(crops) / columns)
    atlas = Image.new("RGBA", (cw * columns, ch * rows))
    contact = Image.new("RGB", (cw * columns, (ch + 28) * rows), "#d3d4d6")
    output.mkdir(parents=True, exist_ok=True)
    (output / "frames").mkdir(exist_ok=True)
    (output / "parts").mkdir(exist_ok=True)
    editable = {"frames": [], "puppet": (layout or {}).get("puppet", {})}
    boxes = [b for _, b, _ in crops]
    ctx = {
        "cw": cw, "ch": ch, "padding": padding, "columns": columns,
        "output": output, "atlas": atlas, "contact": contact,
        "draw": ImageDraw.Draw(contact), "editable": editable,
        "boxes": boxes, "kind": kind, "layout": layout,
        "sheet_bg": _background_candidates(np.array(image)[:, :, :3], tolerance),
        "median_area": float(np.median([(b[2] - b[0]) * (b[3] - b[1]) for b in boxes])),
    }
    frames = [
        _build_frame_entry(i, item, box, crop, ctx)
        for i, (item, box, crop) in enumerate(crops)
    ]
    atlas.save(output / "atlas.png")
    contact.save(output / "contact_sheet.jpg")
    (output / "runtime.mjs").write_text(RUNTIME)
    (output / "layout.review.json").write_text(json.dumps(editable, indent=2))
    atlas_meta = {
        "file": "atlas.png",
        "size": list(atlas.size),
        "columns": columns,
        "rows": rows,
        "padding": padding,
    }
    return frames, atlas_meta, (cw, ch, padding)


def _build_exploded_puppet(frames, output, layout, w, h, cw, ch, padding):
    defaults = {
        "torso": (0.5, 0.65),
        "head": (0.5, 0.35),
        "ears": (0.5, 0.15),
        "arm_L": (0.68, 0.44),
        "arm_R": (0.32, 0.44),
        "leg_L": (0.60, 0.72),
        "leg_R": (0.40, 0.72),
        "prop_backpack": (0.5, 0.47),
    }
    puppet = {"size": [cw, ch * 2], "anchor": [cw / 2, ch * 2 - padding], "parts": {}}
    slots = {s["id"]: s for s in json.loads(TEMPLATE.read_text())["slots"]}
    overrides = (layout or {}).get("puppet", {})
    for frame in frames:
        name, slot = frame["label"], slots.get(frame["label"])
        with Image.open(output / frame["file"]) as part_image:
            bounds = part_image.getchannel("A").getbbox()
        if bounds is None:
            continue
        px, py = slot["pivot"] if slot else (0.5, 0.5)
        pivot = [
            bounds[0] + px * (bounds[2] - bounds[0]),
            bounds[1] + py * (bounds[3] - bounds[1]),
        ]
        sb = frame.get("sourceBox", [0, 0, w, h])
        bx, by = defaults.get(
            name,
            (((sb[0] + sb[2]) * 0.5) / max(1, w), ((sb[1] + sb[3]) * 0.5) / max(1, h)),
        )
        override = overrides.get(name, {})
        puppet["parts"][name] = {
            "file": frame["file"],
            "size": frame["size"],
            "parent": override.get("parent", slot["parent"] if slot else None),
            "pivot": override.get("pivot", pivot),
            "bindPosition": override.get("bindPosition", [bx * cw, by * ch * 2]),
            "status": "ready" if override.get("complete") else "needs_review",
            "visiblePixelsOnly": False,
            "inferred": not bool(override.get("complete")),
        }
    return puppet


def _write_action_set_manifests(result, frames, output):
    action_sets = []
    for row in result.get("animation", {}).get("rows", []):
        selected = [frames[index] for index in row["frames"]]
        subset = {
            **result,
            "frames": selected,
            "animation": {
                **row,
                "template": row["id"],
                "rows": [],
                "frameOrder": list(range(len(selected))),
                "continuousCycle": row.get("loop", False),
            },
        }
        filename = re.sub(r"[^a-zA-Z0-9_-]", "_", row["id"]) + ".manifest.json"
        (output / filename).write_text(json.dumps(subset, indent=2))
        action_sets.append({"action": row["id"], "manifest": filename, "frames": len(selected)})
    return action_sets


def _default_animation_actions(frames, present):
    return {
        "continuousCycle": False,
        "actions": {
            name: [f["index"] for f in frames if f["label"] == name]
            for name in sorted(present)
        },
    }


def _expected_frame_count(layout, kind, definitions):
    if layout:
        return len(definitions)
    return 5 if kind == "turnaround" else None


def _assemble_sheet_manifest(source, size, kind, layout, frames, atlas_meta, meta):
    w, h = size
    layout_dict = layout or {}
    present = {f["label"] for f in frames}
    flagged = [f["index"] for f in frames if f["qa"]["issues"]]
    warnings = meta["warnings"]
    if flagged:
        warnings.append(f"Frames {flagged} have cut issues; see frames[].qa.issues.")
    missing = (
        [a for a in ACTIONS if a not in present]
        if kind == "action" and not layout_dict.get("animation")
        else []
    )
    return {
        "schemaVersion": 1,
        "sourceImage": str(source),
        "sourceSize": [w, h],
        "sheetType": kind,
        "status": "needs_review",
        "spriteStatus": "exported",
        "mattingEngine": meta["matting_engine"],
        "measuredGrid": layout_dict.get("measuredGrid"),
        "vectorCraftAvailable": bool(meta["vc_bin"]),
        "frames": frames,
        "atlas": atlas_meta,
        "coordinates": "top-left pixel origin; sourceBox right/bottom exclusive; anatomical L/R; Three UV origin bottom-left",
        "alignment": "common scale; centered x, bottom aligned; explicit per-frame offsets supported",
        "partsStatus": "needs_review",
        "missingActions": missing,
        "missingParts": meta["missing_parts"],
        "review": {
            "found": len(frames),
            "expected": _expected_frame_count(layout, kind, meta["definitions"]),
            "expectedAtLeast": meta["caption_count"],
            "flaggedFrames": flagged,
        },
        "animation": layout_dict.get("animation") or _default_animation_actions(frames, present),
        "warnings": warnings,
        "contactSheet": "contact_sheet.jpg",
        "editableLayout": "layout.review.json",
        "explodedTemplate": str(TEMPLATE),
    }


def cut_sheet(source, out_dir, kind="auto", layout=None, use_ocr=True, tolerance=25):
    source, output = Path(source).resolve(), Path(out_dir).resolve()
    if not source.is_file():
        raise FileNotFoundError(source)
    if not 1 <= tolerance <= 100:
        raise ValueError("tolerance must be in 1..100")
    with Image.open(source) as src:
        image = ImageOps.exif_transpose(src).convert("RGBA")
    kind = _resolve_sheet_kind(source, kind)
    layout = _resolve_sheet_layout(source, image, layout)
    warnings = list((layout or {}).get("warnings", []))
    definitions, caption_count = _discover_definitions(
        source, image, kind, layout, use_ocr, tolerance, warnings
    )
    if not definitions:
        raise ValueError("No foreground frames specified")
    if len(definitions) > 64:
        raise ValueError("Over 64 frames detected; supply --layout")
    pc_bin = _find_image_cli()
    if not pc_bin:
        raise RuntimeError(
            "photocraft-cli is required to remove backgrounds; install it or put it on PATH"
        )
    crops, missing_parts, matting_engine = _extract_crops(
        image, definitions, kind, layout, pc_bin
    )
    frames, atlas_meta, (cw, ch, padding) = _export_frames_and_atlas(
        image, crops, kind, layout, tolerance, output
    )
    result = _assemble_sheet_manifest(
        source, image.size, kind, layout, frames, atlas_meta,
        {
            "definitions": definitions,
            "caption_count": caption_count,
            "warnings": warnings,
            "missing_parts": missing_parts,
            "matting_engine": matting_engine,
            "vc_bin": _find_vector_cli(),
        },
    )
    if kind == "exploded":
        w, h = image.size
        result["puppet"] = _build_exploded_puppet(
            frames, output, layout, w, h, cw, ch, padding
        )
    result["actionSets"] = _write_action_set_manifests(result, frames, output)
    (output / "manifest.json").write_text(json.dumps(result, indent=2))
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "source",
        nargs="?",
        help="Image, cut directory (with --recut), or directory of vault sheets",
    )
    parser.add_argument("--out-dir", default="exports/pose_and_parts")
    parser.add_argument(
        "--kind",
        choices=["auto", "action", "turnaround", "single", "exploded"],
        default="auto",
    )
    parser.add_argument("--layout", help="JSON source pixel boxes, polygons and pivots")
    parser.add_argument("--no-ocr", action="store_true")
    parser.add_argument("--tolerance", type=int, default=25)
    parser.add_argument(
        "--prompt", metavar="CHARACTER", help="Print exact exploded sheet prompt"
    )
    parser.add_argument("--description", default="")
    parser.add_argument(
        "--recut",
        action="store_true",
        help="Recut flagged frames from an existing cut directory",
    )
    args = parser.parse_args(argv)
    if args.prompt:
        print(exploded_prompt(args.prompt, args.description))
        return 0
    if not args.source:
        parser.error("source or --prompt required")
    src, out = Path(args.source), Path(args.out_dir)
    if args.recut:
        r = recut_flagged_sheet(
            src, out if args.out_dir != "exports/pose_and_parts" else None
        )
        print(
            json.dumps(
                {
                    "manifest": str(
                        (out if args.out_dir != "exports/pose_and_parts" else src)
                        / "manifest.json"
                    ),
                    "frames": len(r["frames"]),
                    "recutFrames": r.get("recutFrames", []),
                    "status": r["status"],
                },
                indent=2,
            )
        )
        return 0
    r = cut_sheet(src, out, args.kind, args.layout, not args.no_ocr, args.tolerance)
    print(
        json.dumps(
            {
                "manifest": str(out / "manifest.json"),
                "frames": len(r["frames"]),
                "mattingEngine": r.get("mattingEngine"),
                "status": r["status"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
