#!/usr/bin/env python3
"""Prepare cutout artwork and its editable native EffectCraft composition."""

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
from pipeline.effectcraft.cutout import build_effectcraft_cutout_animation
from pipeline.blender_cutout_rig import export_blender_python_script


def build_guided_cutout_rig(
    rgba_path: str | Path,
    character_id: str,
    out_dir: Optional[str | Path] = None,
    mode: str = "sway",
    speed: float = 1.0,
    duration: float = 3.0,
    selected_parts: Optional[List[str]] = None,
    out_mp4: Optional[str | Path] = None,
) -> Dict[str, Any]:
    """Run Stage 3A -> Stage 3B -> Stage 3C sequentially."""
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

    # 2. Stage 3C: EffectCraft rig composition & video render
    ec_res = build_effectcraft_cutout_animation(
        rig_plan,
        pose_lib,
        target_dir,
        mode=mode,
        speed=speed,
        duration=duration,
        out_mp4=Path(out_mp4) if out_mp4 else None,
    )
    rig_plan["effectcraft"] = ec_res

    rig_plan_path = target_dir / f"{character_id}_cutout_rig_plan.json"
    pose_lib_path = target_dir / f"{character_id}_pose_library.json"
    rig_plan_path.write_text(json.dumps(rig_plan, indent=2), encoding="utf-8")

    # 3. Stage 3B: VectorCraft SVG tracing
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
        "effectcraft": ec_res,
    }


def export_existing(args):
    path = Path(args.rig_plan).resolve()
    if args.trace_svg_only:
        return trace_part_layers_with_vectorcraft(path, preset=args.trace_preset)
    plan = json.loads(path.read_text())
    library = plan.get("pose_library", {})
    output = Path(args.out_dir).resolve() if args.out_dir else path.parent
    if args.blender_py_only:
        target = (
            Path(args.out_blender_py).resolve()
            if args.out_blender_py
            else output / f"{plan['character_id']}_blender_rig.py"
        )
        export_blender_python_script(
            plan,
            library,
            target,
            mode=args.mode,
            speed=args.speed,
            duration=args.duration,
            glb_path=args.glb_path,
            blend_path=args.blend_path,
        )
        return dict(ok=True, blender_py_path=str(target))
    result = build_effectcraft_cutout_animation(
        plan,
        library,
        output,
        mode=args.mode,
        speed=args.speed,
        duration=args.duration,
        out_mp4=Path(args.out_mp4) if args.out_mp4 else None,
    )
    plan["effectcraft"] = result
    path.write_text(json.dumps(plan, indent=2) + "\n")
    return dict(ok=True, effectcraft=result)


def main(argv: Optional[List[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", nargs="?")
    parser.add_argument("--character")
    parser.add_argument("--out-dir")
    parser.add_argument("--mode", default="sway")
    parser.add_argument("--speed", type=float, default=1.0)
    parser.add_argument("--duration", type=float, default=3.0)
    parser.add_argument("--parts")
    parser.add_argument("--rig-plan")
    parser.add_argument("--render-only", action="store_true")
    parser.add_argument("--blender-py-only", action="store_true")
    parser.add_argument("--trace-svg-only", action="store_true")
    parser.add_argument("--trace-preset", default="16 Colors")
    parser.add_argument("--out-mp4")
    parser.add_argument("--out-blender-py")
    parser.add_argument("--glb-path")
    parser.add_argument("--blend-path")
    args = parser.parse_args(argv)
    if args.rig_plan:
        result = export_existing(args)
    else:
        if args.render_only or args.blender_py_only or args.trace_svg_only:
            parser.error("Exporting an existing rig requires --rig-plan")
        if not args.image or not args.character:
            parser.error("Provide an image and --character")
        result = build_guided_cutout_rig(
            args.image,
            args.character,
            args.out_dir,
            args.mode,
            args.speed,
            args.duration,
            args.parts.split(",") if args.parts else None,
            args.out_mp4,
        )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
