"""Stage 1b: QA review, part polygon masking, and automated sheet recutting for pose_and_part_cutter."""

from __future__ import annotations

import json
import math
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage as ndi

TEMPLATE = (
    Path(__file__).resolve().parent / "templates" / "exploded_part_sheet_spec.json"
)
PARTS = ["head", "ears", "torso", "arm_L", "arm_R", "leg_L", "leg_R", "prop_backpack"]

RUNTIME = """// Pixel units. Serve this folder over HTTP. Three.js is supplied by caller.
export async function loadManifest(url) {
  const r = await fetch(url); if (!r.ok) throw Error(`Manifest HTTP ${r.status}`);
  const m = await r.json(); m.baseURL = new URL('.', new URL(url, location.href)); return m;
}
export function threeFrame(THREE, texture, manifest, index=0) {
  const f = manifest.frames[index], [w,h] = f.size;
  const map = texture.clone(); map.needsUpdate = true;
  map.repeat.set(f.uv[2], f.uv[3]); map.offset.set(f.uv[0], f.uv[1]);
  const sprite = new THREE.Sprite(new THREE.SpriteMaterial({map, transparent:true, depthWrite:false}));
  sprite.scale.set(w,h,1); sprite.center.set(f.anchor[0]/w, 1-f.anchor[1]/h); return sprite;
}
export function p5Frame(p, atlas, manifest, index, x, y) {
  const f=manifest.frames[index], [sx,sy,sw,sh]=f.atlasRect;
  p.push(); p.imageMode(p.CORNER);
  p.image(atlas, x-f.anchor[0], y-f.anchor[1], sw,sh, sx,sy,sw,sh); p.pop();
}
"""


def background_candidates(rgb: np.ndarray, tolerance: int) -> np.ndarray:
    h, w = rgb.shape[:2]
    smooth = ndi.gaussian_filter(rgb.astype(np.float32), (1.2, 1.2, 0))
    pixels = smooth[:: max(1, h // 200), :: max(1, w // 200)].reshape(-1, 3)
    quantized = (pixels / 16).astype(int)
    values, counts = np.unique(quantized, axis=0, return_counts=True)
    modal = values[counts.argmax()]
    bg = np.median(pixels[np.all(quantized == modal, axis=1)], axis=0)
    return np.max(np.abs(smooth - bg), axis=2) <= tolerance


def _is_valid_component_box(
    area: int, bw: int, bh: int, w: int, h: int, min_h_frac: float
) -> bool:
    if area < h * w * 0.0012 or bh < h * min_h_frac or bw < w * 0.018:
        return False
    return bw <= w * 0.55 and bh <= h * 0.94


def _group_boxes_into_rows(
    boxes: list[tuple[int, int, int, int]], h: int
) -> list[tuple[int, int, int, int]]:
    boxes.sort(key=lambda b: (b[1] + b[3]) / 2)
    rows: list[list] = []
    for box in boxes:
        cy = (box[1] + box[3]) / 2
        row = next(
            (
                r
                for r in rows
                if abs(cy - np.median([(b[1] + b[3]) / 2 for b in r])) < h * 0.15
            ),
            None,
        )
        if row is None:
            rows.append([box])
        else:
            row.append(box)
    return [b for row in rows for b in sorted(row, key=lambda b: b[0])]


def detect_components(mask: np.ndarray, kind: str) -> list[tuple[int, int, int, int]]:
    h, w = mask.shape
    labels, _ = ndi.label(mask)
    boxes = []
    areas = np.bincount(labels.ravel())
    min_h_frac = 0.28 if kind == "turnaround" else 0.105
    for i, obj in enumerate(ndi.find_objects(labels), 1):
        if obj is None:
            continue
        ys, xs = obj
        if _is_valid_component_box(
            int(areas[i]), xs.stop - xs.start, ys.stop - ys.start, w, h, min_h_frac
        ):
            boxes.append((xs.start, ys.start, xs.stop, ys.stop))
    return _group_boxes_into_rows(boxes, h)


def _check_detached_pieces_issue(visible: np.ndarray) -> str | None:
    labels, count = ndi.label(visible, structure=np.ones((3, 3)))
    if count <= 1:
        return None
    areas = np.bincount(labels.ravel())[1:]
    main = int(areas.argmax())
    min_piece = max(40, 0.002 * areas[main])
    pieces = [k for k in range(count) if k != main and areas[k] >= min_piece]
    if not pieces:
        return None
    stray_pct = 100.0 * (areas.sum() - areas[main]) / areas.sum()
    return (
        f"{len(pieces)} detached piece(s) besides the figure, {stray_pct:.1f}% of visible pixels "
        "outside it: check for source text, background residue or neighbour fragments"
    )


def frame_review(
    crop: Image.Image,
    index: int,
    boxes: list,
    median_area: float,
    background: np.ndarray,
) -> list[str]:
    issues = []
    x0, y0, x1, y1 = boxes[index]
    overlaps = [
        j
        for j, (a0, b0, a1, b1) in enumerate(boxes)
        if j != index and min(x1, a1) > max(x0, a0) and min(y1, b1) > max(y0, b0)
    ]
    if overlaps:
        issues.append(
            f"box overlaps frame(s) {overlaps}: crop may contain part of a neighbour"
        )
    area = (x1 - x0) * (y1 - y0)
    if median_area and area > 1.8 * median_area:
        issues.append(
            f"box is {area / median_area:.1f}x the median frame: may hold more than one figure"
        )
    elif median_area and area < 0.45 * median_area:
        issues.append(
            f"box is {area / median_area:.2f}x the median frame: may be a fragment or partial figure"
        )
    visible = np.array(crop)[:, :, 3] > 20
    detached_issue = _check_detached_pieces_issue(visible)
    if detached_issue:
        issues.append(detached_issue)
    kept = visible & background[: visible.shape[0], : visible.shape[1]]
    kept_pct = 100.0 * kept.sum() / max(1, visible.sum())
    if kept_pct > 9:
        issues.append(
            f"{kept_pct:.0f}% of visible pixels match the sheet background: background was not removed"
        )
    return issues


def _validate_part_polygon_and_pivot(
    name: str, spec: dict, source_box
) -> tuple[list, list]:
    polygon = spec.get("polygon", [])
    if len(polygon) < 3 or any(
        len(p) != 2 or any(not math.isfinite(v) for v in p) for p in polygon
    ):
        raise ValueError(f"Invalid polygon for {name}")
    x0, y0, x1, y1 = source_box
    if any(not (x0 <= x <= x1 and y0 <= y <= y1) for x, y in polygon):
        raise ValueError(f"{name} polygon outside frame source box")
    pivot = spec.get("pivot")
    if not pivot or len(pivot) != 2 or any(not math.isfinite(v) for v in pivot):
        raise ValueError(f"Invalid pivot for {name}")
    return polygon, pivot


def _extract_single_part(
    name: str,
    spec: dict | None,
    image: Image.Image,
    source_box,
    frame_offset,
    parents: dict,
    output: Path,
    index: int,
) -> dict:
    if not spec:
        return {
            "status": "unavailable",
            "reason": "No isolated artwork or explicit mask",
        }
    polygon, pivot = _validate_part_polygon_and_pivot(name, spec, source_box)
    x0, y0, _, _ = source_box
    mask_img = Image.new("L", image.size)
    ImageDraw.Draw(mask_img).polygon(
        [(x - x0, y - y0) for x, y in polygon], fill=255
    )
    arr = np.array(image)
    arr[:, :, 3] = np.minimum(arr[:, :, 3], np.array(mask_img))
    part = Image.fromarray(arr)
    bounds = part.getchannel("A").getbbox()
    if not bounds:
        return {
            "status": "unavailable",
            "reason": "Mask contains no visible pixels",
        }
    px, py = bounds[:2]
    filename = f"parts/{index:02d}_{name}.png"
    part.crop(bounds).save(output / filename)
    ox, oy = frame_offset
    return {
        "file": filename,
        "size": [bounds[2] - px, bounds[3] - py],
        "status": "ready"
        if spec.get("complete") and not spec.get("inferred")
        else "needs_review",
        "parent": spec.get("parent", parents.get(name)),
        "sourcePivot": list(pivot),
        "pivot": [pivot[0] - x0 - px, pivot[1] - y0 - py],
        "bindPosition": [pivot[0] - x0 + ox, pivot[1] - y0 + oy],
        "sourcePolygon": polygon,
        "visiblePixelsOnly": not bool(spec.get("complete")),
        "inferred": bool(spec.get("inferred")),
    }


def save_parts(
    image: Image.Image,
    source_box,
    frame_offset,
    definitions: dict,
    output: Path,
    index: int,
    template_path: Path = TEMPLATE,
) -> dict:
    parents = {
        s["id"]: s["parent"] for s in json.loads(template_path.read_text())["slots"]
    }
    has_biped_slot = any(k in PARTS for k in definitions)
    part_names = (
        (list(PARTS) + [k for k in definitions if k not in PARTS])
        if has_biped_slot
        else list(definitions)
    )
    return {
        name: _extract_single_part(
            name,
            definitions.get(name),
            image,
            source_box,
            frame_offset,
            parents,
            output,
            index,
        )
        for name in part_names
    }


def write_vector_limb_masks_svg(
    frame_file: str,
    size: tuple[int, int],
    source_box,
    frame_offset,
    definitions: dict,
    output: Path,
    index: int,
) -> str:
    cw, ch = size
    x0, y0, _, _ = source_box
    ox, oy = frame_offset
    svg_rel = f"parts/{index:02d}_limb_masks.svg"
    polys = []
    for name, spec in definitions.items():
        polygon = spec.get("polygon", [])
        if len(polygon) < 3:
            continue
        pts = " ".join(f"{px - x0 + ox:.1f},{py - y0 + oy:.1f}" for px, py in polygon)
        piv = spec.get("pivot", [x0, y0])
        piv_x, piv_y = piv[0] - x0 + ox, piv[1] - y0 + oy
        polys.append(
            f'  <g id="limb_{name}" data-part="{name}" data-pivot="{piv_x:.1f},{piv_y:.1f}">\n'
            f'    <clipPath id="clip_{name}"><polygon points="{pts}"/></clipPath>\n'
            f'    <polygon id="mask_path_{name}" points="{pts}" fill="none" stroke="#ff7a18" stroke-width="1.5" stroke-dasharray="4,3"/>\n'
            f'    <circle cx="{piv_x:.1f}" cy="{piv_y:.1f}" r="3.5" fill="#38bdf8"/>\n'
            f"  </g>"
        )
    svg_text = (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {cw} {ch}" width="{cw}" height="{ch}">\n'
        f'  <image id="felt_artwork" href="../{frame_file}" x="0" y="0" width="{cw}" height="{ch}"/>\n'
        + "\n".join(polys)
        + "\n</svg>\n"
    )
    (output / svg_rel).write_text(svg_text)
    return svg_rel


def _append_detached_piece_exclusions(
    cut_dir: Path, frame_meta: dict, edit_item: dict, x0: int, y0: int, x1: int, y1: int
) -> None:
    frame_img_path = cut_dir / frame_meta["file"]
    if not frame_img_path.is_file():
        return
    with Image.open(frame_img_path) as fim:
        arr = np.array(fim.convert("RGBA"))
    ox, oy = frame_meta.get("sourceOffset", [0, 0])
    bw, bh = x1 - x0, y1 - y0
    crop_alpha = arr[oy : oy + bh, ox : ox + bw, 3] > 20
    labels, count = ndi.label(crop_alpha, structure=np.ones((3, 3)))
    if count <= 1:
        return
    areas = np.bincount(labels.ravel())[1:]
    main = int(areas.argmax()) + 1
    exclusions = list(edit_item.get("excludePolygons", []))
    for comp_id in range(1, count + 1):
        if comp_id == main:
            continue
        ys, xs = np.nonzero(labels == comp_id)
        if len(xs) >= 10:
            sx0, sy0 = int(x0 + xs.min()), int(y0 + ys.min())
            sx1, sy1 = int(x0 + xs.max() + 1), int(y0 + ys.max() + 1)
            exclusions.append([[sx0, sy0], [sx1, sy0], [sx1, sy1], [sx0, sy1]])
    if exclusions:
        edit_item["excludePolygons"] = exclusions


def recut_flagged_sheet(
    cut_dir: Path, out_dir: Path | None = None, expand_px: int = 8
) -> dict:
    """Inspect manifest.json + layout.review.json for QA issues and recut flagged frames cleanly."""
    from pipeline.pose_and_part_cutter import cut_sheet

    cut_dir = Path(cut_dir).resolve()
    manifest_path = cut_dir / "manifest.json"
    review_path = cut_dir / "layout.review.json"
    if not manifest_path.is_file() or not review_path.is_file():
        raise FileNotFoundError(
            f"manifest.json and layout.review.json required in {cut_dir}"
        )
    manifest = json.loads(manifest_path.read_text())
    editable = json.loads(review_path.read_text())
    w, h = manifest["sourceSize"]
    repaired_frames = []

    for frame_meta, edit_item in zip(manifest["frames"], editable["frames"]):
        qa = frame_meta.get("qa", {})
        issues = qa.get("issues", [])
        touches = qa.get("touchesCropBoundary", False)
        if not issues and not touches:
            continue
        x0, y0, x1, y1 = edit_item["box"]
        if touches:
            x0, y0 = max(0, x0 - expand_px), max(0, y0 - expand_px)
            x1, y1 = min(w, x1 + expand_px), min(h, y1 + expand_px)
            edit_item["box"] = [x0, y0, x1, y1]
        if any("detached piece" in iss for iss in issues):
            _append_detached_piece_exclusions(
                cut_dir, frame_meta, edit_item, x0, y0, x1, y1
            )
        repaired_frames.append(frame_meta["index"])

    target_dir = Path(out_dir).resolve() if out_dir else cut_dir
    updated_layout = target_dir / "layout.recut.json"
    target_dir.mkdir(parents=True, exist_ok=True)
    updated_layout.write_text(json.dumps(editable, indent=2))
    res = cut_sheet(
        manifest["sourceImage"],
        target_dir,
        kind=manifest["sheetType"],
        layout=editable,
        use_ocr=False,
    )
    res["recutFrames"] = repaired_frames
    (target_dir / "manifest.json").write_text(json.dumps(res, indent=2))
    return res
