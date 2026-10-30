#!/usr/bin/env python3
"""Prepare reviewed cutout artwork with PhotoCraft and VectorCraft."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Dict, List, Optional

from pipeline.step_photocraft_parts import (
    discover_character_joints_and_polygons,
    mask_and_export_layers_with_photocraft,
    render_cutout_asset_sheet,
)
from pipeline.step_vector_trace import trace_part_layers_with_vectorcraft


def build_guided_cutout_rig(
    rgba_path: str | Path,
    character_id: str,
    out_dir: Optional[str | Path] = None,
    selected_parts: Optional[List[str]] = None,
) -> Dict[str, Any]:
    """Slice reviewed parts, save their rig plan, then trace them."""
    src_path = Path(rgba_path).resolve()
    if not src_path.exists():
        raise FileNotFoundError(f"Input image not found: {src_path}")
    target_dir = Path(out_dir).resolve() if out_dir else src_path.parent
    target_dir.mkdir(parents=True, exist_ok=True)

    # 1. Stage 3A: PhotoCraft part slicing
    discovery = discover_character_joints_and_polygons(
        src_path, character_id=character_id, selected_parts=selected_parts
    )
    pc_res = mask_and_export_layers_with_photocraft(
        src_path, discovery, target_dir, character_id
    )
    pose_lib = discovery.get("pose_library", {})

    joints_map = {
        p["id"]: {
            "pivot": p["pivot"],
            "pivot_uv": p["pivot_uv"],
            "rotation": p["rotation"],
            "joint_radius_px": p["joint_radius_px"],
        }
        for p in pc_res["parts"]
    }
    rig_plan: Dict[str, Any] = {
        "character_id": character_id,
        "rig_type": "cutout_rig",
        "discovery_mode": discovery.get("discovery_mode", "photocraft_guided"),
        "selected_parts": discovery.get("selected_parts", []),
        "source_image": str(src_path),
        "canvas_size": discovery["canvas_size"],
        "character_bbox": discovery["character_bbox"],
        "photocraft_psd": pc_res["psd_path"],
        "parts": pc_res["parts"],
        "joints": joints_map,
        "pose_library": pose_lib,
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
    }
    sheet_path = target_dir / f"{character_id}_cutout_asset_sheet.png"
    render_cutout_asset_sheet(src_path, rig_plan, sheet_path)
    rig_plan["cutout_asset_sheet"] = str(sheet_path)

    rig_plan_path = target_dir / f"{character_id}_cutout_rig_plan.json"
    pose_lib_path = target_dir / f"{character_id}_pose_library.json"
    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")

    # Trace the prepared parts with the installed VectorCraft tool.
    try:
        vc_res = trace_part_layers_with_vectorcraft(rig_plan_path)
        if vc_res.get("ok"):
            rig_plan = json.loads(rig_plan_path.read_text(encoding="utf-8"))
            rig_plan["vectorcraft"] = vc_res
    except Exception as exc:
        rig_plan["vectorcraft"] = {"ok": False, "error": str(exc)}

    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")
    pose_lib_path.write_text(json.dumps(pose_lib, indent=2), encoding="utf-8")

    return {
        "ok": True,
        "character_id": character_id,
        "discovery_mode": rig_plan["discovery_mode"],
        "selected_parts": rig_plan["selected_parts"],
        "rig_plan_path": str(rig_plan_path),
        "pose_library_path": str(pose_lib_path),
        "cutout_asset_sheet": str(sheet_path),
        "photocraft_psd": pc_res["psd_path"],
        "part_count": len(pc_res["parts"]),
        "qa_telemetry": pc_res.get("qa_telemetry", {}),
    }


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image")
    parser.add_argument("--character", required=True)
    parser.add_argument("--out-dir")
    parser.add_argument("--parts")
    args = parser.parse_args(argv)
    result = build_guided_cutout_rig(
        args.image,
        args.character,
        args.out_dir,
        args.parts.split(",") if args.parts else None,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
