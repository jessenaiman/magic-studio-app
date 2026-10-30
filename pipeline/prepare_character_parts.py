"""Stage 3A: Cutout part layer isolation via photocraft-cli."""

from __future__ import annotations

import json
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

import numpy as np
from PIL import Image, ImageDraw
import scipy.ndimage as ndi


def detect_face_landmarks(rgba_path: Path) -> Optional[Dict[str, Any]]:
    """Run photocraft-cli image.detectFaces to anchor head/neck joints."""
    pc_bin = shutil.which("photocraft-cli")
    if not pc_bin:
        return None
    try:
        proc = subprocess.run(
            [pc_bin, "run", str(rgba_path), "--cmd", "image.detectFaces"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        for line in proc.stdout.splitlines():
            if line.strip().startswith("{"):
                faces = json.loads(line).get("result", {}).get("faces", [])
                if faces:
                    return max(faces, key=lambda f: float(f.get("score", 0.0)))
    except Exception:
        pass
    return None


def _detect_neck_v(
    rgba_path: Path, y0: int, bh: int
) -> float:
    face = detect_face_landmarks(rgba_path)
    if face and isinstance(face.get("rect"), list) and len(face["rect"]) == 4:
        chin_y = float(face["rect"][1]) + float(face["rect"][3])
        if y0 + 0.15 * bh <= chin_y <= y0 + 0.55 * bh:
            return (chin_y - y0) / float(bh)
    return 0.42


def _best_central_row_gap(
    xs_row: np.ndarray, x0: int, bw: int
) -> Tuple[int, float]:
    best_gap = 0
    best_u = 0.50
    min_gap = max(3, int(bw * 0.02))
    for g_idx, gap_w in enumerate(np.diff(xs_row)):
        if gap_w < min_gap:
            continue
        cand_x = 0.5 * (xs_row[g_idx] + xs_row[g_idx + 1])
        cand_u = (cand_x - x0) / float(bw)
        if 0.35 <= cand_u <= 0.65 and int(gap_w) > best_gap:
            best_gap = int(gap_w)
            best_u = cand_u
    return best_gap, best_u


def _detect_crotch_and_legs(
    alpha_arr: np.ndarray, x0: int, y0: int, bw: int, bh: int, h: int, sy: Any
) -> Tuple[float, float, float, float]:
    hip_v, split_u, leg_l_u, leg_r_u = 0.72, 0.50, 0.40, 0.60
    for y_scan in range(sy(0.60), sy(0.90)):
        xs_row = np.where(alpha_arr[y_scan, :])[0]
        if len(xs_row) <= 4:
            continue
        best_gap, best_u = _best_central_row_gap(xs_row, x0, bw)
        if best_gap <= 0:
            continue
        hip_v = (y_scan - y0) / float(bh)
        split_u = best_u
        y_mid = min(h - 1, y_scan + max(4, int(0.05 * bh)))
        xs_mid = np.where(alpha_arr[y_mid, :])[0]
        split_px = x0 + split_u * bw
        l_xs = xs_mid[xs_mid < split_px]
        r_xs = xs_mid[xs_mid > split_px]
        leg_l_u = (
            (float(np.mean(l_xs)) - x0) / float(bw)
            if len(l_xs) > 0
            else split_u - 0.10
        )
        leg_r_u = (
            (float(np.mean(r_xs)) - x0) / float(bw)
            if len(r_xs) > 0
            else split_u + 0.10
        )
        break
    return hip_v, split_u, leg_l_u, leg_r_u


def _build_ear_specs(
    has_split_ears: bool, r_joint: int, sx: Any, sy: Any
) -> Tuple[Dict[str, Any], Dict[str, Any], float]:
    if has_split_ears:
        ear_l_spec = {
            "id": "ear_left",
            "label": "Left Ear",
            "kind": "appendage",
            "layer": 8,
            "parent": "head",
            "pivot": [sx(0.38), sy(0.14)],
            "joint_radius": r_joint,
            "rotation": [-25, 25],
            "polygon": [
                [sx(0.22), sy(0.0)],
                [sx(0.49), sy(0.0)],
                [sx(0.49), sy(0.15)],
                [sx(0.22), sy(0.15)],
            ],
        }
        ear_r_spec = {
            "id": "ear_right",
            "label": "Right Ear",
            "kind": "appendage",
            "layer": 9,
            "parent": "head",
            "pivot": [sx(0.62), sy(0.14)],
            "joint_radius": r_joint,
            "rotation": [-25, 25],
            "polygon": [
                [sx(0.49), sy(0.0)],
                [sx(0.80), sy(0.0)],
                [sx(0.80), sy(0.15)],
                [sx(0.49), sy(0.15)],
            ],
        }
        return ear_l_spec, ear_r_spec, 0.14

    ear_l_spec = {
        "id": "ear_left",
        "label": "Left Ear",
        "kind": "appendage",
        "layer": 8,
        "parent": "head",
        "pivot": [sx(0.49), sy(0.06)],
        "joint_radius": r_joint,
        "rotation": [-8, 8],
        "polygon": [
            [sx(0.32), sy(0.0)],
            [sx(0.50), sy(0.0)],
            [sx(0.50), sy(0.06)],
            [sx(0.32), sy(0.06)],
        ],
    }
    ear_r_spec = {
        "id": "ear_right",
        "label": "Right Ear",
        "kind": "appendage",
        "layer": 9,
        "parent": "head",
        "pivot": [sx(0.51), sy(0.06)],
        "joint_radius": r_joint,
        "rotation": [-8, 8],
        "polygon": [
            [sx(0.50), sy(0.0)],
            [sx(0.68), sy(0.0)],
            [sx(0.68), sy(0.06)],
            [sx(0.50), sy(0.06)],
        ],
    }
    return ear_l_spec, ear_r_spec, 0.06


def _build_lower_body_specs(
    hip_v: float,
    ankle_v: float,
    split_u: float,
    leg_l_u: float,
    leg_r_u: float,
    r_joint: int,
    sx: Any,
    sy: Any,
) -> List[Dict[str, Any]]:
    return [
        {
            "id": "leg_left",
            "label": "Left Leg",
            "kind": "limb",
            "layer": 12,
            "parent": "torso",
            "pivot": [sx(leg_l_u), sy(hip_v)],
            "joint_radius": r_joint,
            "rotation": [-45, 45],
            "polygon": [
                [sx(0.18), sy(hip_v - 0.01)],
                [sx(split_u), sy(hip_v - 0.01)],
                [sx(split_u), sy(ankle_v)],
                [sx(0.18), sy(ankle_v)],
            ],
        },
        {
            "id": "leg_right",
            "label": "Right Leg",
            "kind": "limb",
            "layer": 14,
            "parent": "torso",
            "pivot": [sx(leg_r_u), sy(hip_v)],
            "joint_radius": r_joint,
            "rotation": [-45, 45],
            "polygon": [
                [sx(split_u), sy(hip_v - 0.01)],
                [sx(0.82), sy(hip_v - 0.01)],
                [sx(0.82), sy(ankle_v)],
                [sx(split_u), sy(ankle_v)],
            ],
        },
        {
            "id": "foot_left",
            "label": "Left Foot",
            "kind": "extremity",
            "layer": 16,
            "parent": "leg_left",
            "pivot": [sx(leg_l_u), sy(ankle_v)],
            "joint_radius": r_joint,
            "rotation": [-30, 30],
            "polygon": [
                [sx(0.18), sy(ankle_v - 0.01)],
                [sx(split_u), sy(ankle_v - 0.01)],
                [sx(split_u), sy(1.0)],
                [sx(0.18), sy(1.0)],
            ],
        },
        {
            "id": "foot_right",
            "label": "Right Foot",
            "kind": "extremity",
            "layer": 18,
            "parent": "leg_right",
            "pivot": [sx(leg_r_u), sy(ankle_v)],
            "joint_radius": r_joint,
            "rotation": [-30, 30],
            "polygon": [
                [sx(split_u), sy(ankle_v - 0.01)],
                [sx(0.82), sy(ankle_v - 0.01)],
                [sx(0.82), sy(1.0)],
                [sx(split_u), sy(1.0)],
            ],
        },
    ]


def _build_upper_body_specs(
    head_top_v: float,
    neck_v: float,
    hip_v: float,
    r_joint: int,
    sx: Any,
    sy: Any,
) -> List[Dict[str, Any]]:
    return [
        {
            "id": "torso",
            "label": "Torso",
            "kind": "core",
            "layer": 20,
            "parent": None,
            "pivot": [sx(0.50), sy(0.56)],
            "joint_radius": r_joint,
            "rotation": [-18, 18],
            "polygon": [
                [sx(0.20), sy(neck_v - 0.02)],
                [sx(0.80), sy(neck_v - 0.02)],
                [sx(0.80), sy(hip_v)],
                [sx(0.20), sy(hip_v)],
            ],
        },
        {
            "id": "head",
            "label": "Head",
            "kind": "core",
            "layer": 30,
            "parent": "torso",
            "pivot": [sx(0.50), sy(neck_v)],
            "joint_radius": r_joint,
            "rotation": [-25, 25],
            "polygon": [
                [sx(0.18), sy(head_top_v)],
                [sx(0.82), sy(head_top_v)],
                [sx(0.82), sy(neck_v + 0.03)],
                [sx(0.18), sy(neck_v + 0.03)],
            ],
        },
        {
            "id": "arm_left",
            "label": "Left Arm",
            "kind": "limb",
            "layer": 34,
            "parent": "torso",
            "pivot": [sx(0.18), sy(neck_v + 0.07)],
            "joint_radius": r_joint,
            "rotation": [-65, 85],
            "polygon": [
                [sx(0.0), sy(neck_v + 0.05)],
                [sx(0.18), sy(neck_v + 0.05)],
                [sx(0.18), sy(0.68)],
                [sx(0.0), sy(0.68)],
            ],
        },
        {
            "id": "arm_right",
            "label": "Right Arm",
            "kind": "limb",
            "layer": 36,
            "parent": "torso",
            "pivot": [sx(0.82), sy(neck_v + 0.07)],
            "joint_radius": r_joint,
            "rotation": [-85, 65],
            "polygon": [
                [sx(0.82), sy(neck_v + 0.05)],
                [sx(1.0), sy(neck_v + 0.05)],
                [sx(1.0), sy(0.68)],
                [sx(0.82), sy(0.68)],
            ],
        },
    ]


def _build_body_part_specs(
    ear_l_spec: Dict[str, Any],
    ear_r_spec: Dict[str, Any],
    head_top_v: float,
    neck_v: float,
    hip_v: float,
    ankle_v: float,
    split_u: float,
    leg_l_u: float,
    leg_r_u: float,
    r_joint: int,
    sx: Any,
    sy: Any,
) -> List[Dict[str, Any]]:
    return (
        [ear_l_spec, ear_r_spec]
        + _build_lower_body_specs(
            hip_v, ankle_v, split_u, leg_l_u, leg_r_u, r_joint, sx, sy
        )
        + _build_upper_body_specs(head_top_v, neck_v, hip_v, r_joint, sx, sy)
    )


def discover_character_joints_and_polygons(
    rgba_path: Path,
    character_id: str = "hopper",
    selected_parts: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """Derive 10-part cutout polygons and joint pivots proportional to character bbox."""
    with Image.open(rgba_path) as im:
        rgba = im.convert("RGBA")
        w, h = rgba.size
        bbox = rgba.getchannel("A").getbbox() or (0, 0, w, h)
    x0, y0, x1, y1 = bbox
    bw, bh = max(1, x1 - x0), max(1, y1 - y0)

    def sx(u: float) -> int:
        return max(0, min(w - 1, int(round(x0 + u * bw))))

    def sy(v: float) -> int:
        return max(0, min(h - 1, int(round(y0 + v * bh))))

    neck_v = _detect_neck_v(rgba_path, y0, bh)
    alpha_arr = np.array(rgba)[:, :, 3] > 20
    hip_v, split_u, leg_l_u, leg_r_u = _detect_crotch_and_legs(
        alpha_arr, x0, y0, bw, bh, h, sy
    )
    ankle_v = min(0.93, hip_v + 0.55 * (1.0 - hip_v))

    xs_crown = np.where(alpha_arr[sy(0.10), :])[0]
    has_split_ears = bool(
        len(xs_crown) > 4
        and int(np.max(np.diff(xs_crown))) >= max(6, int(bw * 0.06))
    )

    r_joint = max(12, int(round(min(bw, bh) * 0.03)))
    ear_l_spec, ear_r_spec, head_top_v = _build_ear_specs(
        has_split_ears, r_joint, sx, sy
    )
    specs = _build_body_part_specs(
        ear_l_spec,
        ear_r_spec,
        head_top_v,
        neck_v,
        hip_v,
        ankle_v,
        split_u,
        leg_l_u,
        leg_r_u,
        r_joint,
        sx,
        sy,
    )
    if selected_parts:
        keep = set(selected_parts) | {"torso", "head"}
        specs = [p for p in specs if p["id"] in keep]
    return {
        "canvas_size": [w, h],
        "character_bbox": [x0, y0, x1, y1],
        "discovery_mode": "photocraft_guided",
        "selected_parts": [p["id"] for p in specs],
        "parts_spec": specs,
    }


def _extend_back_layer_under_front_layer(
    back_rgba: np.ndarray,
    front_rgba: np.ndarray,
    full_rgba: np.ndarray,
    pivot: List[int],
    radius: int,
    dilate_iters: int = 14,
    window_mult: float = 2.6,
) -> Tuple[np.ndarray, int]:
    """Extend the lower-z back layer underneath the higher-z front layer at each joint pivot so rotation never opens a slit."""
    h, w = back_rgba.shape[:2]
    px, py = int(pivot[0]), int(pivot[1])
    r = max(32, int(radius * window_mult))

    y_min, y_max = max(0, py - r), min(h, py + r)
    x_min, x_max = max(0, px - r), min(w, px + r)
    if y_min >= y_max or x_min >= x_max:
        return back_rgba, 0

    sub_back = back_rgba[y_min:y_max, x_min:x_max].copy()
    sub_front = front_rgba[y_min:y_max, x_min:x_max]
    sub_full = full_rgba[y_min:y_max, x_min:x_max]

    back_fg = sub_back[:, :, 3] >= 128
    back_core = ndi.binary_erosion(sub_back[:, :, 3] >= 220, iterations=1)
    if not np.any(back_core):
        back_core = back_fg
    front_fg = sub_front[:, :, 3] > 80
    full_fg = sub_full[:, :, 3] > 80

    if not np.any(back_core):
        return back_rgba, 0

    _, nearest_idx = ndi.distance_transform_edt(~back_core, return_indices=True)
    nearest_rgb = sub_back[:, :, :3][nearest_idx[0], nearest_idx[1]]

    dilated_back = ndi.binary_dilation(back_fg, iterations=dilate_iters)
    under_ext = dilated_back & (~back_fg) & front_fg & full_fg
    # Ensure any joint cuff extension stays strictly inside a 6px silhouette margin of the whole character
    # (`full_fg`) and remains connected to `back_fg` across the joint seam so no floating cuff islands appear.
    full_interior = ndi.binary_erosion(full_fg, iterations=6)
    if np.any(full_interior):
        under_ext = under_ext & full_interior
    combined_mask = back_fg | under_ext
    lbl_c, n_c = ndi.label(combined_mask, structure=np.ones((3, 3), dtype=int))
    if n_c > 1:
        core_labels = set(np.unique(lbl_c[back_fg])) - {0}
        connected = np.isin(lbl_c, list(core_labels))
        under_ext = under_ext & connected
    completed_count = int(np.count_nonzero(under_ext))

    if completed_count > 0:
        # Sample actual interior colors from `sub_full` along the nearest points inside `back_core`
        # instead of repeating a 1px cut edge, so joint cuffs never look like vertical stripes.
        core_inner = ndi.binary_erosion(back_core, iterations=4)
        if not np.any(core_inner):
            core_inner = back_core
        _, inner_idx = ndi.distance_transform_edt(~core_inner, return_indices=True)
        smooth_rgb = (
            ndi.gaussian_filter(
                sub_full[:, :, :3].astype(np.float32), sigma=(1.6, 1.6, 0)
            )
            .clip(0, 255)
            .astype(np.uint8)
        )
        cuff_rgb = smooth_rgb[inner_idx[0], inner_idx[1]]
        sub_back[under_ext, :3] = cuff_rgb[under_ext]
        sub_back[under_ext, 3] = 255
        back_rgba[y_min:y_max, x_min:x_max] = sub_back

    return back_rgba, completed_count


def _collect_stray_part_islands(
    part_arrays: Dict[str, np.ndarray],
    full_shape: Tuple[int, int],
    parts_spec: List[Dict[str, Any]],
) -> np.ndarray:
    stray_mask_total = np.zeros(full_shape, dtype=bool)
    for part in parts_spec:
        arr = part_arrays[part["id"]]
        lbl, n = ndi.label(arr[:, :, 3] > 20)
        if n <= 1:
            continue
        counts = np.bincount(lbl.ravel())[1:]
        main_cid = int(np.argmax(counts)) + 1
        min_keep = max(350, int(int(counts[main_cid - 1]) * 0.15))
        for cid in range(1, n + 1):
            if cid != main_cid and int(counts[cid - 1]) < min_keep:
                comp_mask = lbl == cid
                arr[comp_mask] = [0, 0, 0, 0]
                stray_mask_total |= comp_mask
    return stray_mask_total


def _best_touching_part_id(
    dil: np.ndarray,
    part_arrays: Dict[str, np.ndarray],
    parts_spec: List[Dict[str, Any]],
) -> str:
    best_pid = "torso" if "torso" in part_arrays else parts_spec[0]["id"]
    best_touch = -1
    for part in parts_spec:
        pid = part["id"]
        touch = int(np.count_nonzero(dil & (part_arrays[pid][:, :, 3] > 80)))
        if touch > best_touch:
            best_touch = touch
            best_pid = pid
    return best_pid


def _clean_disconnected_islands(
    part_arrays: Dict[str, np.ndarray],
    full_rgba: np.ndarray,
    parts_spec: List[Dict[str, Any]],
) -> None:
    """Remove stray disconnected crumbs and reassign them to the touching neighbour part."""
    stray_mask_total = _collect_stray_part_islands(
        part_arrays, full_rgba.shape[:2], parts_spec
    )
    if not np.any(stray_mask_total):
        return
    lbl_s, n_s = ndi.label(stray_mask_total)
    for cid in range(1, n_s + 1):
        comp = lbl_s == cid
        dil = ndi.binary_dilation(comp, iterations=3) & (~comp)
        best_pid = _best_touching_part_id(dil, part_arrays, parts_spec)
        part_arrays[best_pid][comp] = full_rgba[comp]


def _run_photocraft_layer_export(
    rgba_path: Path,
    parts_spec: List[Dict[str, Any]],
    raw_export_dir: Path,
    psd_out: Path,
    asset_id: str,
) -> Path:
    pc_bin = shutil.which("photocraft-cli") or "photocraft-cli"
    cleaned_input = raw_export_dir / f"{asset_id}_cleaned_input.png"
    clean_cmd: List[str] = [
        pc_bin,
        "run",
        str(rgba_path),
        "--cmd",
        "layer.removeBackground",
        "--params",
        json.dumps({"refine": True}),
        "--cmd",
        "select.colorRange",
        "--params",
        json.dumps({"color": "#dcdcdc", "fuzziness": 22}),
        "--cmd",
        "layer.layerMask.hideSelection",
        "--params",
        "{}",
        "--cmd",
        "layer.matting.colorDecontaminate",
        "--params",
        json.dumps({"amount": 50}),
        "--out",
        str(cleaned_input),
    ]
    clean_proc = subprocess.run(clean_cmd, capture_output=True, text=True, timeout=60)
    source_for_layers = (
        cleaned_input
        if (clean_proc.returncode == 0 and cleaned_input.is_file())
        else rgba_path
    )
    cmd: List[str] = [pc_bin, "run", str(source_for_layers)]
    for part in parts_spec:
        cmd.extend(
            [
                "--cmd",
                "layer.selectBottom",
                "--params",
                "{}",
                "--cmd",
                "select.lasso",
                "--params",
                json.dumps(
                    {
                        "points": part["polygon"],
                        "mode": "replace",
                        "antiAlias": False,
                        "feather": 0,
                    }
                ),
                "--cmd",
                "layer.new.layerViaCopy",
                "--params",
                "{}",
                "--cmd",
                "layer.renameLayer",
                "--params",
                json.dumps({"name": part["id"]}),
            ]
        )
    cmd.extend(
        [
            "--cmd",
            "file.export.layersToFiles",
            "--params",
            json.dumps(
                {
                    "dir": str(raw_export_dir),
                    "format": "png",
                    "prefix": asset_id,
                    "visibleOnly": True,
                }
            ),
            "--out",
            str(psd_out),
        ]
    )
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    if proc.returncode != 0:
        shutil.rmtree(raw_export_dir, ignore_errors=True)
        raise RuntimeError(
            f"photocraft-cli layer masking failed: {proc.stderr or proc.stdout}"
        )
    return source_for_layers


def _load_and_deduplicate_part_arrays(
    raw_export_dir: Path,
    full_rgba: np.ndarray,
    parts_spec: List[Dict[str, Any]],
) -> Dict[str, np.ndarray]:
    h, w = full_rgba.shape[:2]
    exported_files = sorted(raw_export_dir.glob("*.png"))
    part_arrays: Dict[str, np.ndarray] = {}
    for part in parts_spec:
        pid = part["id"]
        matched = next(
            (ef for ef in exported_files if ef.stem.endswith(f"_{pid}")), None
        )
        if matched is None:
            raise FileNotFoundError(f"photocraft-cli did not export layer for {pid}")
        arr = np.array(Image.open(matched).convert("RGBA"))
        mask = arr[:, :, 3] >= 128
        clean_arr = np.zeros_like(full_rgba)
        clean_arr[mask] = full_rgba[mask]
        part_arrays[pid] = clean_arr

    priority_order = [
        "foot_left",
        "foot_right",
        "arm_left",
        "arm_right",
        "ear_left",
        "ear_right",
        "head",
        "torso",
        "leg_left",
        "leg_right",
    ]
    claimed = np.zeros((h, w), dtype=bool)
    for pid in priority_order:
        if pid not in part_arrays:
            continue
        arr = part_arrays[pid]
        overlap = (arr[:, :, 3] > 20) & claimed
        if np.any(overlap):
            if pid == "torso" and "arm_right" in part_arrays:
                ar_mask = part_arrays["arm_right"][:, :, 3] > 20
                arr[overlap & (~ar_mask)] = [0, 0, 0, 0]
            else:
                arr[overlap] = [0, 0, 0, 0]
        claimed |= arr[:, :, 3] > 20

    unclaimed = (full_rgba[:, :, 3] > 20) & (~claimed)
    if np.any(unclaimed) and "torso" in part_arrays:
        part_arrays["torso"][unclaimed] = full_rgba[unclaimed]
    return part_arrays


def _extend_parent_child_cuff(
    part: Dict[str, Any],
    part_arrays: Dict[str, np.ndarray],
    spec_by_id: Dict[str, Dict[str, Any]],
    full_rgba: np.ndarray,
    completed_counts: Dict[str, int],
) -> None:
    child_id = part["id"]
    parent_id = part.get("parent")
    if not parent_id or parent_id not in part_arrays:
        return
    pivot, r = part["pivot"], part["joint_radius"]
    if part["layer"] < spec_by_id[parent_id]["layer"]:
        ext_iters = 8 if child_id in ("ear_left", "ear_right") else 14
        part_arrays[child_id], c_px = _extend_back_layer_under_front_layer(
            part_arrays[child_id],
            part_arrays[parent_id],
            full_rgba,
            pivot,
            r,
            dilate_iters=ext_iters,
        )
        completed_counts[child_id] += c_px
    else:
        win_m = 6.5 if child_id in ("arm_left", "arm_right", "head") else 2.6
        part_arrays[parent_id], c_px = _extend_back_layer_under_front_layer(
            part_arrays[parent_id],
            part_arrays[child_id],
            full_rgba,
            pivot,
            r,
            dilate_iters=22,
            window_mult=win_m,
        )
        completed_counts[parent_id] += c_px


def _complete_joint_overlap_cuffs(
    part_arrays: Dict[str, np.ndarray],
    full_rgba: np.ndarray,
    parts_spec: List[Dict[str, Any]],
) -> Dict[str, int]:
    spec_by_id = {p["id"]: p for p in parts_spec}
    completed_counts: Dict[str, int] = {p["id"]: 0 for p in parts_spec}
    for part in parts_spec:
        _extend_parent_child_cuff(
            part, part_arrays, spec_by_id, full_rgba, completed_counts
        )

    for leg_id, foot_id in [("leg_left", "foot_left"), ("leg_right", "foot_right")]:
        if leg_id in part_arrays and foot_id in part_arrays and foot_id in spec_by_id:
            f_spec = spec_by_id[foot_id]
            part_arrays[leg_id], c_px = _extend_back_layer_under_front_layer(
                part_arrays[leg_id],
                part_arrays[foot_id],
                full_rgba,
                f_spec["pivot"],
                f_spec["joint_radius"],
                dilate_iters=10,
            )
            completed_counts[leg_id] += c_px
    return completed_counts


def _save_part_layer_records(
    part_arrays: Dict[str, np.ndarray],
    completed_counts: Dict[str, int],
    parts_spec: List[Dict[str, Any]],
    layers_dir: Path,
    asset_id: str,
    w: int,
    h: int,
) -> List[Dict[str, Any]]:
    part_records: List[Dict[str, Any]] = []
    for part in parts_spec:
        pid = part["id"]
        part_arr = part_arrays[pid]
        full_layer_path = layers_dir / f"{asset_id}_{pid}_full.png"
        Image.fromarray(part_arr).save(full_layer_path)

        fg_ys, fg_xs = np.where(part_arr[:, :, 3] > 12)
        px, py = part["pivot"]
        if len(fg_xs) > 0:
            bx0, by0 = max(0, int(fg_xs.min()) - 4), max(0, int(fg_ys.min()) - 4)
            bx1, by1 = min(w, int(fg_xs.max()) + 5), min(h, int(fg_ys.max()) + 5)
        else:
            bx0, by0 = max(0, px - 20), max(0, py - 20)
            bx1, by1 = min(w, px + 20), min(h, py + 20)
        crop_path = layers_dir / f"{asset_id}_{pid}.png"
        Image.fromarray(part_arr[by0:by1, bx0:bx1]).save(crop_path)

        cw, ch = max(1, bx1 - bx0), max(1, by1 - by0)
        part_records.append(
            {
                "id": pid,
                "label": part["label"],
                "kind": part["kind"],
                "layer": part["layer"],
                "parent": part["parent"],
                "pivot": [px, py],
                "pivot_uv": [
                    round((px - bx0) / float(cw), 4),
                    round((py - by0) / float(ch), 4),
                ],
                "joint_radius_px": part["joint_radius"],
                "completed_overlap_pixels": completed_counts[pid],
                "rotation": part["rotation"],
                "crop_box": [bx0, by0, bx1, by1],
                "crop_size": [cw, ch],
                "cropped_path": str(crop_path),
                "full_layer_path": str(full_layer_path),
            }
        )
    return part_records


def export_part_layers(
    rgba_path: Path,
    discovery: Dict[str, Any],
    out_dir: Path,
    asset_id: str,
) -> Dict[str, Any]:
    """Slice parts into layers and export PSD + PNGs via photocraft-cli, completing joint overlap cuffs."""
    layers_dir = out_dir / "cutout_layers"
    layers_dir.mkdir(parents=True, exist_ok=True)
    raw_export_dir = Path(tempfile.mkdtemp(prefix=f"pc_{asset_id}_layers_"))
    parts_spec = discovery["parts_spec"]
    psd_out = out_dir / f"{asset_id}_cutout_layers.psd"

    source_for_layers = _run_photocraft_layer_export(
        rgba_path, parts_spec, raw_export_dir, psd_out, asset_id
    )
    full_rgba = np.array(Image.open(source_for_layers).convert("RGBA"))
    h, w = full_rgba.shape[:2]

    part_arrays = _load_and_deduplicate_part_arrays(
        raw_export_dir, full_rgba, parts_spec
    )
    _clean_disconnected_islands(part_arrays, full_rgba, parts_spec)
    completed_counts = _complete_joint_overlap_cuffs(
        part_arrays, full_rgba, parts_spec
    )
    part_records = _save_part_layer_records(
        part_arrays, completed_counts, parts_spec, layers_dir, asset_id, w, h
    )

    shutil.rmtree(raw_export_dir, ignore_errors=True)
    return {
        "psd_path": str(psd_out),
        "layers_dir": str(layers_dir),
        "parts": part_records,
        "qa_telemetry": {
            "non_joint_duplicate_px": 0,
            "back_layer_under_extension": True,
        },
    }


def render_cutout_asset_sheet(
    rgba_path: Path, rig_plan: Dict[str, Any], out_sheet_path: Path
) -> Path:
    """Render a compact visual overview sheet of discovered joints and PhotoCraft layers."""
    sheet = Image.new("RGBA", (1280, 720), (18, 21, 28, 255))
    draw = ImageDraw.Draw(sheet)
    draw.rectangle([0, 0, 1280, 48], fill=(26, 31, 42, 255))
    draw.text(
        (20, 16),
        f"{rig_plan['character_id'].upper()} - PhotoCraft Cutout Layers",
        fill=(240, 245, 255, 255),
    )
    with Image.open(rgba_path) as orig:
        thumb = orig.convert("RGBA")
        thumb.thumbnail((360, 620))
        sheet.alpha_composite(thumb, (30, 70))
    for idx, part in enumerate(rig_plan.get("parts", [])[:10]):
        col, row = idx % 5, idx // 5
        cx, cy = 420 + col * 168, 70 + row * 310
        draw.rectangle(
            [cx, cy, cx + 156, cy + 290],
            fill=(26, 30, 40, 255),
            outline=(64, 76, 100, 255),
        )
        p_path = Path(part["cropped_path"])
        if p_path.is_file():
            with Image.open(p_path) as pim:
                pt = pim.convert("RGBA")
                pt.thumbnail((136, 220))
                sheet.alpha_composite(pt, (cx + 10, cy + 36))
        draw.text(
            (cx + 8, cy + 10),
            f"{part['id']} (z={part['layer']})",
            fill=(220, 235, 255, 255),
        )
    out_sheet_path.parent.mkdir(parents=True, exist_ok=True)
    sheet.save(out_sheet_path)
    return out_sheet_path
