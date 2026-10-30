#!/usr/bin/env python3
"""Stage 4A: 2D Turnaround + Motion Range -> 3D Visual Hull (`visual_hull.npz`).

First sequential stage of the refactored 3D+2D animation pipeline:
reads 2D turnaround views + measured 2D `motion_range.json` (`footLiftPx`)
and carves a closed 3D visual hull (`visual_hull.npz`) with view-aligned projection textures.
"""

from __future__ import annotations

import json
import math
import sys
from pathlib import Path
from typing import Any, Dict

_REPO_ROOT = str(Path(__file__).resolve().parents[1])
if _REPO_ROOT not in sys.path:
    sys.path.insert(0, _REPO_ROOT)

from pipeline.character_iteration_exporter import (
    CHARACTER_PRESETS,
    POSE_PARTS_ROOT,
    SHARE_ROOT,
)

ROOT = Path(__file__).resolve().parents[1]
SCHOOL_BG = Path("/home/jesse/main-vault/Creative/Old MacDonald/oldmac-school.png")
CLASS_BG = Path("/home/jesse/main-vault/Creative/old-macs-class.png")

SIX_CAST_ORDER = [
    "miss_hayley",
    "old_mac",
    "hopper",
    "whiskers",
    "puddles",
    "rusty",
]

CHARACTER_STAGE_SPECS: Dict[str, Dict[str, Any]] = {
    "miss_hayley": {
        "role": "teacher",
        "height_m": 1.82,
        "action_style": "teacher_welcome",
        "start_xy": (-2.10, 0.88),
        "end_xy": (-2.00, 0.82),
        "start_yaw_deg": 16.0,
        "facing_yaw_deg": 12.0,
    },
    "old_mac": {
        "role": "teacher",
        "height_m": 1.84,
        "action_style": "principal_beckon",
        "start_xy": (2.10, 0.88),
        "end_xy": (2.00, 0.82),
        "start_yaw_deg": -16.0,
        "facing_yaw_deg": -12.0,
    },
    "hopper": {
        "role": "student",
        "height_m": 1.34,
        "action_style": "bunny_hop_walk",
        "start_xy": (-1.78, -0.85),
        "end_xy": (-1.26, -0.15),
        "start_yaw_deg": 22.0,
        "facing_yaw_deg": 0.0,
    },
    "whiskers": {
        "role": "student",
        "height_m": 1.26,
        "action_style": "cat_walk_wave",
        "start_xy": (-0.78, -1.00),
        "end_xy": (-0.42, -0.15),
        "start_yaw_deg": 14.0,
        "facing_yaw_deg": 0.0,
    },
    "puddles": {
        "role": "student",
        "height_m": 1.20,
        "action_style": "duck_waddle_hop",
        "start_xy": (0.78, -1.00),
        "end_xy": (0.42, -0.15),
        "start_yaw_deg": -14.0,
        "facing_yaw_deg": 0.0,
    },
    "rusty": {
        "role": "student",
        "height_m": 1.40,
        "action_style": "foal_trot_wave",
        "start_xy": (1.78, -0.85),
        "end_xy": (1.26, -0.15),
        "start_yaw_deg": -22.0,
        "facing_yaw_deg": 0.0,
    },
    "jenny": {
        "role": "student",
        "height_m": 1.15,
        "action_style": "chick_hop_wave",
        "start_xy": (0.0, -1.05),
        "end_xy": (0.0, -0.18),
        "start_yaw_deg": 10.0,
        "facing_yaw_deg": 0.0,
    },
}

VIEW_ANGLES: Dict[str, list[int]] = {
    "hopper": [0, -45, -90, 45, 180],
    "whiskers": [0, -45, 90, 135, 180],
    "puddles": [0, -45, -90, 45, 180],
    "rusty": [0, 45, 90, -45, 180],
    "jenny": [0, -45, 90, 45, 180],
    "miss_hayley": [0, -45, -90, 45, 180],
    "old_mac": [0, -45, -90, 45, 180],
}

ANATOMY: Dict[str, Dict[str, float]] = {
    "hopper": dict(
        hip=0.265,
        knee=0.155,
        ankle=0.055,
        neck=0.535,
        shoulder=0.49,
        elbow=0.37,
        wrist=0.29,
        ear=0.79,
        leg_x=0.075,
        arm_x=0.16,
    ),
    "whiskers": dict(
        hip=0.28,
        knee=0.16,
        ankle=0.045,
        neck=0.57,
        shoulder=0.52,
        elbow=0.40,
        wrist=0.32,
        ear=0.89,
        leg_x=0.075,
        arm_x=0.17,
    ),
    "puddles": dict(
        hip=0.19,
        knee=0.12,
        ankle=0.045,
        neck=0.56,
        shoulder=0.49,
        elbow=0.37,
        wrist=0.29,
        ear=0.91,
        leg_x=0.085,
        arm_x=0.16,
    ),
    "rusty": dict(
        hip=0.33,
        knee=0.19,
        ankle=0.055,
        neck=0.60,
        shoulder=0.54,
        elbow=0.43,
        wrist=0.34,
        ear=0.88,
        leg_x=0.065,
        arm_x=0.14,
    ),
    "jenny": dict(
        hip=0.17,
        knee=0.10,
        ankle=0.045,
        neck=0.55,
        shoulder=0.48,
        elbow=0.38,
        wrist=0.30,
        ear=0.90,
        leg_x=0.075,
        arm_x=0.24,
    ),
    "miss_hayley": dict(
        hip=0.36,
        knee=0.19,
        ankle=0.075,
        neck=0.70,
        shoulder=0.64,
        elbow=0.49,
        wrist=0.37,
        ear=0.91,
        leg_x=0.065,
        arm_x=0.165,
    ),
    "old_mac": dict(
        hip=0.28,
        knee=0.16,
        ankle=0.065,
        neck=0.60,
        shoulder=0.55,
        elbow=0.40,
        wrist=0.28,
        ear=0.88,
        leg_x=0.085,
        arm_x=0.19,
    ),
}

PERFORMANCES: Dict[str, Dict[str, Any]] = {
    "hopper": dict(period=10, offset=0, lift=0.80, sway=0.010, gesture="ear_perk"),
    "whiskers": dict(period=12, offset=3, lift=0.55, sway=0.006, gesture="small_wave"),
    "puddles": dict(period=8, offset=1, lift=0.65, sway=0.024, gesture="wing_greeting"),
    "rusty": dict(period=10, offset=2, lift=0.70, sway=0.012, gesture="confident_wave"),
    "jenny": dict(period=8, offset=1, lift=0.70, sway=0.020, gesture="wing_greeting"),
    "miss_hayley": dict(period=12, offset=0, lift=0.0, sway=0.003, gesture="welcome"),
    "old_mac": dict(period=14, offset=4, lift=0.0, sway=0.002, gesture="beckon"),
}


def resolve_character_assets(slug: str, reference_root: Path | None = None) -> dict:
    """Read existing 2D turnaround views and measured 2D motion_range.json."""
    if slug not in CHARACTER_STAGE_SPECS:
        raise ValueError(f"Unknown character: {slug}")
    preset = CHARACTER_PRESETS.get(slug, {"title": slug})
    motion_path = SHARE_ROOT / slug / "motion_range.json"
    motion = json.loads(motion_path.read_text())
    lift = motion.get("limits", {}).get("footLiftPx", {}).get("max")
    still_h = motion.get("still", {}).get("heightPx")
    if (
        lift is None
        or still_h is None
        or not math.isfinite(float(lift))
        or float(still_h) <= 0
    ):
        raise ValueError(
            f"{slug}: measured footLiftPx.max and standing height are required: {motion_path}"
        )
    views = []
    if slug in ("miss_hayley", "old_mac"):
        reference_root = (
            reference_root or ROOT / "exports/farm_school_milestone/references"
        )
        sheet = reference_root / slug / "turnaround.png"
        provenance = reference_root / slug / "provenance.json"
        if not sheet.is_file() or not provenance.is_file():
            raise ValueError(
                f"{slug}: a genuine or explicitly generated five-view sheet with provenance is required at {sheet}"
            )
        for i, (label, angle) in enumerate(
            zip(
                ["front", "quarter_front", "side", "opposite_quarter", "back"],
                VIEW_ANGLES[slug],
            )
        ):
            views.append(dict(label=label, angle=angle, sheet=str(sheet), cell=i))
        provenance_data = json.loads(provenance.read_text())
    else:
        reg_path = SHARE_ROOT / slug / "turnaround_registration.json"
        reg = json.loads(reg_path.read_text())
        for fr, angle in zip(reg["frames"], VIEW_ANGLES[slug]):
            views.append(
                dict(
                    label=fr["label"],
                    angle=angle,
                    file=str(Path(reg["assetRoot"]) / fr["file"]),
                    baseline=fr["registered"]["footBaselineY"],
                    crown=fr["registered"]["crownY"],
                    center=fr["registered"]["footCenterX"],
                )
            )
        provenance_data = dict(
            kind="existing_turnaround_artwork",
            registration=str(reg_path),
            source=json.loads(
                (Path(preset["turnaround_dir"]) / "manifest.json").read_text()
            )["sourceImage"],
        )
    return dict(
        slug=slug,
        title=preset.get("title", slug),
        **CHARACTER_STAGE_SPECS[slug],
        anatomy=ANATOMY[slug],
        performance=PERFORMANCES[slug],
        views=views,
        foot_lift_max_px=float(lift),
        standing_height_px=float(still_h),
        foot_lift_max_m=max(0.0, float(lift))
        / float(still_h)
        * CHARACTER_STAGE_SPECS[slug]["height_m"],
        motion_source=str(motion_path),
        motion_kind=motion.get("kind"),
        provenance=provenance_data,
    )
