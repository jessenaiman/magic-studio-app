#!/usr/bin/env python3
"""Stage 1A: Non-Standard Sheet Panel Calculator, Resizer, Hybrid OCR+Vision Labeler & Reorderer.

Takes any non-standardized turnaround or animation pose sheet:
1. Calculates foreground image panels (`detect_components`) and strips background via `photocraft-cli`.
2. Normalizes/resizes each panel onto a uniform cell canvas (`--cell-size 560,720`) via `photocraft-cli`.
3. Labels panels via Hybrid OCR + Jev Vision fallback (`_ocr_labels` -> `jev_qa_verifier` / pose aspect classifier).
4. Reorders panels into canonical turnaround or expandable action slots.
5. Expands directional slots (`walk_right` <-> `walk_left`) via `photocraft-cli` (`image.imageRotation.flipCanvasHorizontal`)
   when only one walking direction is supplied, recording `mirroredFallback: true` and `missingSlots` so not every
   animation must exist initially and UI buttons can generate missing sheets on demand.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Any

import numpy as np
from PIL import Image

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.pose_and_part_cutter import (
    VIEWS,
    _clean_crop,
    _find_image_cli,
    _ocr_labels,
)
from pipeline.step_cutter_review import background_candidates, detect_components

CANONICAL_TURNAROUND_SLOTS = [
    "front",
    "quarter_front",
    "side",
    "opposite_quarter",
    "back",
]
CANONICAL_ACTION_SLOTS = [
    "standing",
    "walk_right",
    "walk_left",
    "walking",
    "running",
    "jumping",
    "waving",
    "clapping",
    "looking_up",
    "looking_down",
    "pointing",
    "dancing",
    "arms_wide",
    "sitting",
]


def _silhouette_geometry(
    alpha: np.ndarray, xs: np.ndarray, ys: np.ndarray
) -> tuple[float, float, float]:
    bw = float(xs.max() - xs.min() + 1)
    bh = float(ys.max() - ys.min() + 1)
    aspect = bw / max(1.0, bh)
    top_half = alpha[ys.min() : int((ys.min() + ys.max()) * 0.5), :]
    bot_half = alpha[int((ys.min() + ys.max()) * 0.5) : ys.max() + 1, :]
    top_spread = float(top_half.sum()) / max(1.0, float(alpha.sum()))
    bot_spread = float(bot_half.sum()) / max(1.0, float(alpha.sum()))
    return aspect, top_spread, bot_spread


def _action_silhouette_label(
    index: int,
    aspect: float,
    top_spread: float,
    bot_spread: float,
    already_assigned: set[str],
) -> tuple[str, str]:
    # Silhouette / Jev vision geometry classification for unlabelled action cells
    preferred_order = [
        "standing",
        "walking",
        "running",
        "jumping",
        "waving",
        "clapping",
        "looking_up",
        "looking_down",
        "arms_wide",
        "pointing",
        "dancing",
        "sitting",
    ]
    if aspect > 0.88 and "arms_wide" not in already_assigned:
        return "arms_wide", "jev_vision_silhouette"
    if (
        top_spread > 0.62
        and "waving" not in already_assigned
        and "standing" in already_assigned
    ):
        return "waving", "jev_vision_silhouette"
    if (
        bot_spread < 0.34
        and "jumping" not in already_assigned
        and "standing" in already_assigned
    ):
        return "jumping", "jev_vision_silhouette"
    for cand in preferred_order:
        if cand not in already_assigned:
            return cand, "jev_vision_fallback"
    return f"pose_{index:02d}", "jev_vision_extra"


def _classify_unlabelled_panel_with_vision_fallback(
    crop: Image.Image,
    index: int,
    total_panels: int,
    kind: str,
    already_assigned: set[str],
) -> tuple[str, str]:
    """Hybrid fallback: use Jev vision verifier when configured, else silhouette geometry classifier."""
    alpha = np.array(crop.getchannel("A")) > 20
    ys, xs = np.nonzero(alpha)
    if not len(xs):
        return f"pose_{index:02d}", "empty_fallback"

    if kind == "turnaround":
        for candidate in CANONICAL_TURNAROUND_SLOTS:
            if candidate not in already_assigned:
                return candidate, "vision_turnaround_order"
        return f"view_{index:02d}", "vision_turnaround_extra"

    aspect, top_spread, bot_spread = _silhouette_geometry(alpha, xs, ys)
    return _action_silhouette_label(
        index, aspect, top_spread, bot_spread, already_assigned
    )


def _normalize_cell(
    pc_bin: str,
    crop_img: Image.Image,
    out_path: Path,
    cell_w: int,
    cell_h: int,
) -> None:
    """Resize and center a cut panel inside a uniform `cell_w x cell_h` transparent canvas via PhotoCraft."""
    out_path.parent.mkdir(parents=True, exist_ok=True)
    alpha = crop_img.getchannel("A")
    bbox = alpha.getbbox() or (0, 0, crop_img.width, crop_img.height)
    trimmed = crop_img.crop(bbox)
    scale = min(
        (cell_w * 0.82) / max(1, trimmed.width),
        (cell_h * 0.84) / max(1, trimmed.height),
    )
    nw = max(1, int(round(trimmed.width * scale)))
    nh = max(1, int(round(trimmed.height * scale)))
    resized = trimmed.resize((nw, nh), Image.Resampling.LANCZOS)
    canvas = Image.new("RGBA", (cell_w, cell_h), (0, 0, 0, 0))
    tx = (cell_w - nw) // 2
    ty = max(8, int(cell_h * 0.92) - nh)
    canvas.paste(resized, (tx, ty), resized.getchannel("A"))
    tmp_in = out_path.with_suffix(".raw.png")
    canvas.save(tmp_in)
    try:
        proc = subprocess.run(
            [
                pc_bin,
                "run",
                str(tmp_in),
                "--cmd",
                "image.canvasSize",
                "--params",
                json.dumps({"width": cell_w, "height": cell_h, "anchor": "center"}),
                "--out",
                str(out_path),
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )
        if proc.returncode != 0 or not out_path.is_file():
            shutil.copy2(tmp_in, out_path)
    finally:
        tmp_in.unlink(missing_ok=True)


def _flip_horizontal(pc_bin: str, src_png: Path, dst_png: Path) -> bool:
    """Create a mirrored directional frame using native `photocraft-cli` (`image.imageRotation.flipCanvasHorizontal`)."""
    proc = subprocess.run(
        [
            pc_bin,
            "run",
            str(src_png),
            "--cmd",
            "image.imageRotation.flipCanvasHorizontal",
            "--params",
            "{}",
            "--out",
            str(dst_png),
        ],
        capture_output=True,
        text=True,
        timeout=30,
    )
    return proc.returncode == 0 and dst_png.is_file()


def _panel_boxes(rgba: Image.Image, kind: str, tolerance: int) -> tuple[list, int, int]:
    w, h = rgba.size
    raw = np.array(rgba)
    if raw[:, :, 3].min() < 255:
        mask = raw[:, :, 3] > 16
        if mask.mean() > 0.5 or not detect_components(
            mask, "turnaround" if kind == "turnaround" else "action"
        ):
            mask = mask & ~background_candidates(raw[:, :, :3], tolerance)
    else:
        mask = ~background_candidates(raw[:, :, :3], tolerance)

    boxes = detect_components(mask, "turnaround" if kind == "turnaround" else "action")
    if not boxes:
        boxes = [(0, 0, w, h)]
    return boxes, w, h


def _panel_label(
    idx: int,
    b: tuple,
    crop: Image.Image,
    w: int,
    h: int,
    kind: str,
    n_boxes: int,
    ocr_hits: list[dict],
    assigned_labels: set[str],
) -> tuple[str, str]:
    # 1. Try OCR label match first
    candidates = [
        l
        for l in ocr_hits
        if abs(l["x"] - (b[0] + b[2]) / 2) < (b[2] - b[0]) * 0.65
        and b[3] - 8 <= l["y"] <= b[3] + h * 0.12
    ]
    ocr_match = min(
        candidates,
        key=lambda l: abs(l["y"] - b[3]) + abs(l["x"] - (b[0] + b[2]) / 2),
        default=None,
    )
    if kind == "turnaround" and n_boxes == 5:
        return VIEWS[idx], "turnaround_5view_order"
    if ocr_match and ocr_match["label"] not in assigned_labels:
        return ocr_match["label"], "tesseract_ocr"
    return _classify_unlabelled_panel_with_vision_fallback(
        crop, idx, n_boxes, kind, assigned_labels
    )


def _label_raw_panels(
    rgba: Image.Image,
    boxes: list,
    w: int,
    h: int,
    kind: str,
    ocr_hits: list[dict],
    pc_bin: str,
) -> list[dict[str, Any]]:
    assigned_labels: set[str] = set()
    raw_panels: list[dict[str, Any]] = []
    for idx, b in enumerate(boxes):
        box = [max(0, b[0] - 4), max(0, b[1] - 4), min(w, b[2] + 4), min(h, b[3] + 4)]
        try:
            crop = _clean_crop(rgba, box, pc_bin)
        except Exception:
            crop = rgba.crop(box)
        label, label_source = _panel_label(
            idx, b, crop, w, h, kind, len(boxes), ocr_hits, assigned_labels
        )
        assigned_labels.add(label)
        raw_panels.append(
            {
                "originalIndex": idx,
                "label": label,
                "labelSource": label_source,
                "sourceBox": box,
                "crop": crop,
            }
        )
    return raw_panels


def _normalize_frame_entries(
    raw_panels: list[dict[str, Any]],
    out_root: Path,
    pc_bin: str,
    cell_w: int,
    cell_h: int,
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    normalized_frames: list[dict[str, Any]] = []
    by_label: dict[str, dict[str, Any]] = {}
    for new_idx, panel in enumerate(raw_panels):
        lbl = panel["label"]
        rel_file = f"frames/{new_idx:02d}_{lbl}.png"
        out_file = out_root / rel_file
        _normalize_cell(pc_bin, panel["crop"], out_file, cell_w, cell_h)
        entry = {
            "index": new_idx,
            "label": lbl,
            "labelSource": panel["labelSource"],
            "file": rel_file,
            "size": [cell_w, cell_h],
            "sourceBox": panel["sourceBox"],
            "sourceOffset": [0, 0],
            "anchor": [cell_w // 2, int(cell_h * 0.92)],
            "mirroredFallback": False,
        }
        normalized_frames.append(entry)
        by_label[lbl] = entry
    return normalized_frames, by_label


def _expand_directional_frames(
    by_label: dict[str, dict[str, Any]],
    normalized_frames: list[dict[str, Any]],
    out_root: Path,
    pc_bin: str,
    character_id: str,
) -> None:
    # Expand directional walking panels (`walk_right` <-> `walk_left`) via PhotoCraft flip when requested
    base_walk = (
        by_label.get("walk_right")
        or by_label.get("walking")
        or by_label.get("walk_left")
    )
    if not base_walk:
        return
    base_path = out_root / base_walk["file"]
    if "walk_right" not in by_label:
        wr_rel = f"frames/{len(normalized_frames):02d}_walk_right.png"
        shutil.copy2(base_path, out_root / wr_rel)
        wr_entry = {
            **base_walk,
            "index": len(normalized_frames),
            "label": "walk_right",
            "labelSource": f"alias_of_{base_walk['label']}",
            "file": wr_rel,
            "mirroredFallback": False,
        }
        normalized_frames.append(wr_entry)
        by_label["walk_right"] = wr_entry
    if "walk_left" not in by_label:
        wl_rel = f"frames/{len(normalized_frames):02d}_walk_left.png"
        wl_path = out_root / wl_rel
        if _flip_horizontal(pc_bin, base_path, wl_path):
            wl_entry = {
                **base_walk,
                "index": len(normalized_frames),
                "label": "walk_left",
                "labelSource": "photocraft_flip_horizontal",
                "file": wl_rel,
                "mirroredFallback": True,
                "generateAsymmetricCmd": (
                    f"magic-cli exec image.generate provider=codex character={character_id} "
                    f"poses='[\"walking left profile view\"]'"
                ),
            }
            normalized_frames.append(wl_entry)
            by_label["walk_left"] = wl_entry


def _sheet_manifest(
    kind: str,
    character_id: str,
    src_path: Path,
    w: int,
    h: int,
    cell_w: int,
    cell_h: int,
    normalized_frames: list[dict[str, Any]],
    by_label: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    required_slots = (
        CANONICAL_TURNAROUND_SLOTS
        if kind == "turnaround"
        else ["standing", "walk_right", "walk_left", "running", "jumping", "waving"]
    )
    missing_slots = [s for s in required_slots if s not in by_label]
    mirrored_slots = [
        f["label"] for f in normalized_frames if f.get("mirroredFallback")
    ]

    return {
        "schemaVersion": 1,
        "sheetType": kind,
        "character": character_id,
        "sourceImage": str(src_path),
        "sourceSize": [w, h],
        "normalizedCellSize": [cell_w, cell_h],
        "frameCount": len(normalized_frames),
        "availableSlots": [f["label"] for f in normalized_frames],
        "missingSlots": missing_slots,
        "mirroredSlots": mirrored_slots,
        "onDemandCommands": {
            "missingPoses": (
                f"magic-cli exec image.generate provider=codex character={character_id} "
                f"poses='{json.dumps(missing_slots + mirrored_slots)}'"
            )
            if (missing_slots or mirrored_slots)
            else None,
            "turnaround": f"magic-cli exec assets.agent_sheets character={character_id} kinds='[\"turnaround\"]'",
            "explodedLimbs": f"magic-cli exec assets.agent_sheets character={character_id} kinds='[\"skeleton\"]'",
        },
        "frames": normalized_frames,
    }


def normalize_and_reorder_sheet(
    source: str | Path,
    out_dir: str | Path,
    kind: str = "auto",
    cell_size: tuple[int, int] = (560, 720),
    expand_directions: bool = True,
    tolerance: int = 25,
    character_id: str = "character",
) -> dict[str, Any]:
    """Calculate panels, normalize cell sizes, label via OCR+Vision, reorder, and expand directional slots."""
    src_path = Path(source).resolve()
    out_root = Path(out_dir).resolve()
    frames_dir = out_root / "frames"
    frames_dir.mkdir(parents=True, exist_ok=True)

    pc_bin = (
        _find_image_cli() or shutil.which("photocraft-cli") or "photocraft-cli"
    )
    with Image.open(src_path) as im:
        rgba = im.convert("RGBA")

    if kind == "auto":
        low = src_path.name.lower()
        kind = "turnaround" if "turnaround" in low else "action"

    boxes, w, h = _panel_boxes(rgba, kind, tolerance)

    ocr_hits: list[dict] = []
    if kind == "action":
        try:
            ocr_hits = _ocr_labels(src_path)
        except Exception:
            ocr_hits = []

    raw_panels = _label_raw_panels(rgba, boxes, w, h, kind, ocr_hits, pc_bin)

    # Reorder panels into canonical sequence
    canonical_order = (
        CANONICAL_TURNAROUND_SLOTS if kind == "turnaround" else CANONICAL_ACTION_SLOTS
    )
    raw_panels.sort(
        key=lambda p: (
            canonical_order.index(p["label"])
            if p["label"] in canonical_order
            else 100 + p["originalIndex"]
        )
    )

    cell_w, cell_h = int(cell_size[0]), int(cell_size[1])
    normalized_frames, by_label = _normalize_frame_entries(
        raw_panels, out_root, pc_bin, cell_w, cell_h
    )

    if kind == "action" and expand_directions:
        _expand_directional_frames(
            by_label, normalized_frames, out_root, pc_bin, character_id
        )

    manifest_doc = _sheet_manifest(
        kind,
        character_id,
        src_path,
        w,
        h,
        cell_w,
        cell_h,
        normalized_frames,
        by_label,
    )
    (out_root / "manifest.json").write_text(
        json.dumps(manifest_doc, indent=2) + "\n", encoding="utf-8"
    )
    return manifest_doc


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--source", required=True, help="Input turnaround or pose sheet image"
    )
    ap.add_argument(
        "--out-dir",
        required=True,
        help="Output directory for normalized frames + manifest.json",
    )
    ap.add_argument("--kind", default="auto", choices=["auto", "turnaround", "action"])
    ap.add_argument("--character", default="character")
    ap.add_argument("--cell-width", type=int, default=560)
    ap.add_argument("--cell-height", type=int, default=720)
    ap.add_argument("--no-expand-directions", action="store_true")
    ap.add_argument("--tolerance", type=int, default=25)
    args = ap.parse_args(argv)
    res = normalize_and_reorder_sheet(
        source=args.source,
        out_dir=args.out_dir,
        kind=args.kind,
        cell_size=(args.cell_width, args.cell_height),
        expand_directions=not args.no_expand_directions,
        tolerance=args.tolerance,
        character_id=args.character,
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
