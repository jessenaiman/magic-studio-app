#!/usr/bin/env python3
"""Stage 3B: Observed silhouette motion-range measurement & timed trajectory compiler.

Measures each pose frame's crown, foot baseline, lift (`footLiftPx`), and centroid offsets
relative to the standing rest pose, and compiles timed contact/lift/apex_hold/landing/rest
trajectories scaled into rig coordinates (`motion_range.json` + `timed_trajectories.json`).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.step_turnaround_register import (
    SCHEMA_VERSION,
    measure_silhouette,
    read_sheet_frames,
    write_json_atomic,
)

REQUESTED_POSES = (
    "standing",
    "walking",
    "running",
    "jumping",
    "waving",
    "sitting",
    "looking_up",
    "looking_down",
)


def _rows(frames: list[dict]) -> list[dict]:
    """Cluster source boxes by overlapping row centers, not filename indices."""
    groups: list[dict] = []
    for f in sorted(
        frames, key=lambda fr: sum(fr["meta"].get("sourceBox", [0, 0, 0, 0])[1::2]) / 2
    ):
        meta = f["meta"]
        if "sourceBox" not in meta or "sourceOffset" not in meta:
            f["row"] = None
            continue
        box = meta["sourceBox"]
        cy = (box[1] + box[3]) / 2
        height = box[3] - box[1]
        match = next(
            (
                g
                for g in groups
                if abs(cy - g["center"]) < 0.55 * min(height, g["height"])
            ),
            None,
        )
        if match is None:
            match = {"center": cy, "height": height, "frames": []}
            groups.append(match)
        match["frames"].append(f)
        f["row"] = groups.index(match)
    return groups


def _kf(
    t: float,
    phase: str,
    pose: str,
    root_dy: float = 0.0,
    head: float = 0.0,
    arm_l: float = 0.0,
    arm_r: float = 0.0,
    leg_l: float = 0.0,
    leg_r: float = 0.0,
    sec: float = 0.0,
) -> dict:
    return {
        "t": t,
        "phase": phase,
        "pose": pose,
        "rootDyPx": root_dy,
        "headRotDeg": head,
        "armLRotDeg": arm_l,
        "armRRotDeg": arm_r,
        "legLRotDeg": leg_l,
        "legRRotDeg": leg_r,
        "secondaryRotDeg": sec,
    }


def _compile_locomotion_tracks(
    walk_lift_px: float, run_lift_px: float, jump_lift_px: float, poses: dict
) -> dict:
    up_pose = "looking_up" if "looking_up" in poses else "standing"
    down_pose = "looking_down" if "looking_down" in poses else "standing"
    run_pose = "running" if "running" in poses else "jumping"
    return {
        "idle": {
            "duration": 2.0,
            "loop": True,
            "keyframes": [
                _kf(0.0, "rest", "standing"),
                _kf(0.5, "lift", up_pose, -round(walk_lift_px * 0.25, 2), -3.0, 2.5, -2.5, 0.0, 0.0, 4.0),
                _kf(1.0, "rest", "standing", 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -2.0),
                _kf(1.5, "contact", down_pose, round(walk_lift_px * 0.15, 2), 2.5, -2.0, 2.0, 0.0, 0.0, -3.5),
                _kf(2.0, "rest", "standing"),
            ],
        },
        "walk": {
            "duration": 1.6,
            "loop": True,
            "keyframes": [
                _kf(0.0, "contact", "standing", 0.0, 0.0, 16.0, -16.0, -18.0, 18.0, -6.0),
                _kf(0.4, "lift", "walking", -walk_lift_px, -3.5, 0.0, 0.0, 0.0, 0.0, 8.0),
                _kf(0.8, "contact", "standing", 0.0, 0.0, -16.0, 16.0, 18.0, -18.0, 6.0),
                _kf(1.2, "lift", "walking", -walk_lift_px, 3.5, 0.0, 0.0, 0.0, 0.0, -8.0),
                _kf(1.6, "contact", "standing", 0.0, 0.0, 16.0, -16.0, -18.0, 18.0, -6.0),
            ],
        },
        "run": {
            "duration": 1.2,
            "loop": True,
            "keyframes": [
                _kf(0.0, "contact", "walking", 2.0, 2.0, 26.0, -26.0, -28.0, 28.0, -10.0),
                _kf(0.3, "apex_hold", run_pose, -run_lift_px, -4.0, 6.0, -6.0, -8.0, 8.0, 12.0),
                _kf(0.6, "contact", "walking", 2.0, -2.0, -26.0, 26.0, 28.0, -28.0, 10.0),
                _kf(0.9, "apex_hold", run_pose, -run_lift_px, 4.0, -6.0, 6.0, 8.0, -8.0, -12.0),
                _kf(1.2, "contact", "walking", 2.0, 2.0, 26.0, -26.0, -28.0, 28.0, -10.0),
            ],
        },
        "jump": {
            "duration": 1.6,
            "loop": True,
            "keyframes": [
                _kf(0.0, "rest", "standing"),
                _kf(0.3, "contact", "standing", round(jump_lift_px * 0.25, 2), 3.0, 14.0, -14.0, 10.0, -10.0, -6.0),
                _kf(0.75, "apex_hold", "jumping", -jump_lift_px, -5.0, -26.0, 26.0, -14.0, 14.0, 14.0),
                _kf(1.2, "landing", "standing", round(jump_lift_px * 0.2, 2), 2.5, 8.0, -8.0, 8.0, -8.0, -8.0),
                _kf(1.6, "rest", "standing"),
            ],
        },
    }


def _compile_expressive_tracks(walk_lift_px: float, poses: dict) -> dict:
    point_pose = "pointing" if "pointing" in poses else "waving"
    clap_pose = "clapping" if "clapping" in poses else "standing"
    dance_pose = "dancing" if "dancing" in poses else "waving"
    return {
        "wave": {
            "duration": 1.6,
            "loop": True,
            "keyframes": [
                _kf(0.0, "rest", "standing", 0.0, 0.0, 0.0, -24.0, 0.0, 0.0, 0.0),
                _kf(0.4, "lift", "waving", -round(walk_lift_px * 0.35, 2), -5.0, 4.0, -42.0, 0.0, 0.0, 9.0),
                _kf(0.8, "apex_hold", "waving", 0.0, 4.0, -3.0, -16.0, 0.0, 0.0, -7.0),
                _kf(1.2, "lift", "waving", -round(walk_lift_px * 0.35, 2), -4.0, 4.0, -40.0, 0.0, 0.0, 8.0),
                _kf(1.6, "rest", "standing", 0.0, 0.0, 0.0, -24.0, 0.0, 0.0, 0.0),
            ],
        },
        "talk_express": {
            "duration": 1.6,
            "loop": True,
            "keyframes": [
                _kf(0.0, "rest", "standing", 0.0, 0.0, -8.0, 8.0, 0.0, 0.0, 0.0),
                _kf(0.4, "lift", point_pose, -round(walk_lift_px * 0.3, 2), -4.5, -20.0, 18.0, -4.0, 4.0, 7.0),
                _kf(0.8, "contact", clap_pose, 0.0, 3.5, 12.0, -12.0, 0.0, 0.0, -6.0),
                _kf(1.2, "lift", dance_pose, -round(walk_lift_px * 0.35, 2), -3.0, -16.0, 22.0, 4.0, -4.0, 8.0),
                _kf(1.6, "rest", "standing", 0.0, 0.0, -8.0, 8.0, 0.0, 0.0, 0.0),
            ],
        },
    }


def compile_timed_trajectories(
    still_height_px: float, limits: dict, poses: dict
) -> dict:
    """Compile timed contact/lift/hold/rest trajectories scaled from observed pose metrics."""
    foot_lim = limits.get("footLiftPx") or {}
    max_lift_px = float(foot_lim.get("max") or 0.0)
    norm_lift = min(0.28, max(0.04, max_lift_px / max(1.0, float(still_height_px))))
    walk_lift_px = round(max(4.0, still_height_px * norm_lift * 0.45), 2)
    run_lift_px = round(max(7.0, still_height_px * norm_lift * 0.75), 2)
    jump_lift_px = round(max(12.0, still_height_px * norm_lift * 1.15), 2)
    tracks = _compile_locomotion_tracks(walk_lift_px, run_lift_px, jump_lift_px, poses)
    tracks.update(_compile_expressive_tracks(walk_lift_px, poses))
    return {
        "schemaVersion": SCHEMA_VERSION,
        "fps": 24,
        "referenceHeightPx": still_height_px,
        "normalizedLiftEnvelope": round(norm_lift, 4),
        "tracks": tracks,
    }


def _compute_row_baselines(rows: list[dict], np: object) -> list[dict]:
    row_info = []
    for i, row in enumerate(rows):
        grounds = [
            f["measure"]["footBaselineY"]
            - f["meta"]["sourceOffset"][1]
            + f["meta"]["sourceBox"][1]
            for f in row["frames"]
        ]
        row_info.append(
            {
                "row": i,
                "baselineSourceY": float(np.median(grounds)),
                "method": "median measured foot baseline of source-sheet row",
                "poseLabels": [f["meta"]["label"] for f in row["frames"]],
                "baselineSpreadPx": float(max(grounds) - min(grounds)),
            }
        )
    return row_info


def _measure_single_leg_diff(
    frame: dict, top: float, bottom: float, alpha_threshold: int, np: object
) -> float:
    alpha_arr = np.array(frame["image"].getchannel("A")) >= alpha_threshold
    leg_top = int(round(bottom - max(8, (bottom - top) * 0.35)))
    leg_band = alpha_arr[max(0, leg_top) : int(bottom), :]
    yy_fg, xx_fg = np.where(leg_band)
    if len(xx_fg) <= 10:
        return 0.0
    mid_x = 0.5 * (float(xx_fg.min()) + float(xx_fg.max()))
    l_yy = yy_fg[xx_fg < mid_x]
    r_yy = yy_fg[xx_fg >= mid_x]
    if len(l_yy) > 5 and len(r_yy) > 5:
        return float(abs(int(l_yy.max()) - int(r_yy.max())))
    return 0.0


def _compute_frame_values(
    frame: dict,
    row_info: list[dict],
    height: float,
    still_area: float,
    alpha_threshold: int,
    np: object,
) -> tuple[dict, list[float], float, str]:
    m = frame["measure"]
    meta = frame["meta"]
    left, top, right, bottom = m["bbox"]
    anchor_x = float(meta.get("anchor", [frame["image"].width / 2, 0])[0])
    if frame.get("row") is not None:
        r_meta = row_info[frame["row"]]
        ground = r_meta["baselineSourceY"]
        dy = meta["sourceBox"][1] - meta["sourceOffset"][1] - ground
        row_lift = -(bottom + dy)
        leg_diff = 0.0
        if r_meta["baselineSpreadPx"] <= 3.0 and meta.get("label") in (
            "walking",
            "running",
            "jumping",
        ):
            leg_diff = _measure_single_leg_diff(
                frame, top, bottom, alpha_threshold, np
            )
        lift = max(float(row_lift), leg_diff) if row_lift >= 0 else float(row_lift)
        origin = "source_row_median_baseline"
    else:
        dy = -m["footBaselineY"]
        lift = None
        origin = "own_feet; source row unavailable"
    bounds = [left - anchor_x, top + dy, right - anchor_x, bottom + dy]
    metrics = {
        "widthPx": m["widthPx"],
        "heightPx": m["heightPx"],
        "heightRatioToStill": m["heightPx"] / height,
        "areaPx": m["areaPx"],
        "areaRatioToStill": m["areaPx"] / max(1, still_area),
        "leftPx": bounds[0],
        "topPx": bounds[1],
        "rightPx": bounds[2],
        "bottomPx": bounds[3],
        "centroidXPx": m["centroid"][0] - anchor_x,
        "centroidYPx": m["centroid"][1] + dy,
        "footLiftPx": lift,
    }
    return metrics, bounds, dy, origin


def _build_pose_profile(
    frame: dict, height: float, dy: float, alpha_threshold: int, np: object
) -> list[dict]:
    profile = []
    mask = np.array(frame["image"].getchannel("A")) >= alpha_threshold
    cx = float(frame["meta"].get("anchor", [frame["image"].width / 2, 0])[0])
    for h in (0.10, 0.25, 0.50, 0.75, 0.90):
        y = int(round(-h * height - dy))
        xs = np.flatnonzero(mask[y]) if 0 <= y < mask.shape[0] else np.array([])
        profile.append(
            {
                "heightAboveGroundUnits": h,
                "leftUnits": float((xs.min() - cx) / height) if len(xs) else None,
                "rightUnits": float((xs.max() + 1 - cx) / height) if len(xs) else None,
            }
        )
    return profile


def _compute_metric_limits(still_metrics: dict, poses: dict) -> dict:
    limits = {}
    for key, still_val in still_metrics.items():
        observed = [
            (p["metrics"][key], label)
            for label, p in poses.items()
            if p["metrics"][key] is not None
        ]
        if not observed:
            limits[key] = None
            continue
        lo, low_label = min(observed, key=lambda v: v[0])
        hi, high_label = max(observed, key=lambda v: v[0])
        limits[key] = {
            "min": lo,
            "max": hi,
            "minPose": low_label,
            "maxPose": high_label,
            "still": still_val,
            "minDeltaFromStill": lo - still_val if still_val is not None else None,
            "maxDeltaFromStill": hi - still_val if still_val is not None else None,
        }
    return limits


def measure_motion_range(
    poses_dir: str | Path,
    out_dir: str | Path,
    registration: dict | None = None,
    alpha_threshold: int = 128,
    _loaded: tuple[list[dict], dict[str, str]] | None = None,
) -> dict:
    import numpy as np

    frames, hashes = _loaded if _loaded is not None else read_sheet_frames(poses_dir)
    labelled = {}
    for f in frames:
        label = f["meta"]["label"]
        if label in labelled:
            label = f"{label}_{f['meta'].get('index', len(labelled))}"
        labelled[label] = f
        f["measure"] = measure_silhouette(f["image"], alpha_threshold)
    if "standing" not in labelled:
        raise ValueError("Missing standing (STILL) reference")
    still = labelled["standing"]
    height = max(1, still["measure"]["heightPx"])
    still_area = max(1, still["measure"]["areaPx"])
    row_info = _compute_row_baselines(_rows(frames), np)

    still_metrics, _, _, _ = _compute_frame_values(
        still, row_info, height, still_area, alpha_threshold, np
    )
    poses = {}
    for label, frame in labelled.items():
        metrics, bounds, dy, origin = _compute_frame_values(
            frame, row_info, height, still_area, alpha_threshold, np
        )
        poses[label] = {
            "sourceFile": str(frame["path"]),
            "frameIndex": frame["meta"].get("index", 0),
            "sourceMeasurement": frame["measure"],
            "row": frame.get("row"),
            "verticalOrigin": origin,
            "metrics": metrics,
            "deltaFromStill": {
                k: (
                    v - still_metrics[k]
                    if v is not None and still_metrics[k] is not None
                    else None
                )
                for k, v in metrics.items()
            },
            "bboxStandingUnits": [v / height for v in bounds],
            "profile": _build_pose_profile(frame, height, dy, alpha_threshold, np),
            "jointAnglesRadians": None,
        }
    limits = _compute_metric_limits(still_metrics, poses)
    timed_trajectories = compile_timed_trajectories(height, limits, poses)
    target_h = registration["target"]["heightPx"] if registration else None
    result = {
        "schemaVersion": SCHEMA_VERSION,
        "kind": "observed_silhouette_motion_range",
        "alphaThreshold": alpha_threshold,
        "still": {
            "label": "standing",
            "file": str(still["path"]),
            "heightPx": height,
            "metrics": still_metrics,
        },
        "poses": poses,
        "limits": limits,
        "timedTrajectories": timed_trajectories,
        "units": {
            "pixels": "original pose frame pixels; no per-pose resizing",
            "standingUnits": "one standing crown-to-foot silhouette height = 1",
            "axes": "x right, y down; y=0 is estimated row ground; image edges exclusive",
        },
        "requestedPoses": list(REQUESTED_POSES),
        "missingRequestedPoses": [p for p in REQUESTED_POSES if p not in poses],
        "rowBaselines": row_info,
        "jointAngleLimits": None,
        "rootTrajectory": None,
        "sourceHashes": hashes,
        "registrationFile": "turnaround_registration.json" if registration else None,
        "turnaroundHeightPx": target_h,
        "poseToTurnaroundScale": target_h / height if target_h else None,
        "warnings": [
            "These are sampled 2D silhouette envelopes, not physiological joint limits or temporal motion.",
            "Pose artwork includes props/costume and viewpoint changes; envelopes include their pixels.",
            "Row-median ground is a sheet-layout estimate; hovering whole rows cannot establish ground.",
            "Horizontal root translation cannot be recovered from independent centered pose drawings.",
            "CLI, browser and Blender must explicitly read this file; existing consumers were not edited.",
        ],
    }
    out = Path(out_dir)
    write_json_atomic(out / "motion_range.json", result)
    write_json_atomic(out / "timed_trajectories.json", timed_trajectories)
    return result


def load_motion_range(path: str | Path) -> dict:
    value = json.loads(Path(path).read_text())
    if (
        value.get("schemaVersion") != SCHEMA_VERSION
        or value.get("kind") != "observed_silhouette_motion_range"
    ):
        raise ValueError("Unsupported motion range schema")
    return value


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--poses-dir", required=True)
    ap.add_argument("--out-dir", required=True)
    ap.add_argument("--alpha-threshold", type=int, default=128)
    args = ap.parse_args(argv)
    res = measure_motion_range(
        args.poses_dir, args.out_dir, alpha_threshold=args.alpha_threshold
    )
    print(json.dumps(res, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
