"""Native EffectCraft cutout compositions from reviewed parts and authored motion."""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from typing import Any, Dict, Optional

from ..rig_motion import pose_beats
from .scene import SceneBuilder
from .project import NativeProject


def _part_layer(part, pose_library, beats, sway):
    pid, pivot = part["id"], [*part["pivot"], 0.0]
    low, high = part["rotation"]
    keys = [
        [
            time,
            max(
                low,
                min(
                    high,
                    float(
                        pose_library["poses"][pose]
                        .get("parts", {})
                        .get(pid, {})
                        .get("rotation", 0)
                    ),
                ),
            ),
        ]
        for time, pose in beats
    ]
    motion = {"keys": keys}
    if beats and pose_library.get("loop", False):
        motion["expression"] = 'loopOut("cycle")'
    authored = part.get("effectcraft", {})
    properties = {"transform/rotation": motion} if keys else {}
    properties.update(authored.get("properties", {}))
    layer = dict(
        id=pid,
        file=str(Path(part["full_layer_path"]).resolve()),
        anchor=pivot,
        position=pivot,
        properties=properties,
    )
    parent = part.get("parent") or ("Performance" if sway else None)
    if parent:
        layer["parent"] = parent
    layer.update({k: authored[k] for k in ("puppet", "sketch") if k in authored})
    return layer


def _native_cutout_scene(rig_plan, pose_library, mode, speed, duration):
    layers, controls = [], []
    sway = mode in {"sway", "idle"} and mode not in pose_library.get("sequences", {})
    beats = [] if sway or mode == "static" else pose_beats(pose_library, mode, speed)
    if sway:
        width, height = rig_plan["canvas_size"]
        layers.append(
            dict(
                id="Performance",
                null=True,
                position=[width / 2, height / 2, 0],
                anchor=[0, 0, 0],
                properties={
                    "transform/rotation": {
                        "expression": (
                            "Math.sin(time * Math.PI * "
                            'thisComp.layer("Controls").effect("Speed")("Slider")) * '
                            'thisComp.layer("Controls").effect("Sway")("Slider")'
                        )
                    }
                },
            )
        )
    for part in sorted(rig_plan["parts"], key=lambda p: p["layer"]):
        pid = part["id"]
        layer = _part_layer(part, pose_library, beats, sway)
        layers.append(layer)
        controls.extend(
            [
                dict(layer=pid, path="transform/rotation", name=f"{pid}_rotation"),
                dict(layer=pid, media=True, name=f"{pid}_media"),
            ]
        )
    canvas = [int(v) + int(v) % 2 for v in rig_plan["canvas_size"]]
    controllers = (
        [dict(name="Sway", value=1.5), dict(name="Speed", value=speed)] if sway else []
    )
    return dict(
        name=f"{rig_plan['character_id']}_cutout_rig",
        canvas=canvas,
        fps=24,
        duration=duration,
        layers=layers,
        controls=controls,
        controllers=controllers,
    )


def _render_effectcraft(project, output, *options, required=True):
    output.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(  # noqa: S603 -- Native CLI argv, no shell.
        [
            project.executable,
            "--project",
            str(project.path),
            *options,
            "--out",
            str(output),
        ],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if required and result.returncode:
        raise RuntimeError(result.stderr or result.stdout)


def build_effectcraft_cutout_animation(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_dir: Path,
    mode: str = "sway",
    speed: float = 1.0,
    duration: float = 3.0,
    out_mp4: Optional[Path] = None,
) -> Dict[str, Any]:
    from time import time_ns

    cid = rig_plan["character_id"]
    out_dir = Path(out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    duration = max(0.5, float(duration))
    spec = _native_cutout_scene(rig_plan, pose_library, mode, speed, duration)
    source = out_dir / f"{cid}_scene.json"
    source.write_text(json.dumps(spec, indent=2) + "\n")
    built = SceneBuilder(source, out_dir / "native" / str(time_ns())).build()
    project = NativeProject(built["source"])
    values = out_dir / f"{cid}_values.json"
    values.write_text(
        json.dumps(
            {f"{p['id']}_media": p["full_layer_path"] for p in rig_plan["parts"]},
            indent=2,
        )
        + "\n"
    )
    frame0 = out_dir / f"{cid}_ec_cutout_frame0.png"
    frame1 = out_dir / f"{cid}_ec_cutout_frame1.png"
    peak = min(duration * 0.5, 0.5)
    for frame, time in [(frame0, 0), (frame1, peak)]:
        _render_effectcraft(
            project, frame, "render-frame", "--time", str(time), "--transparent"
        )
    alpha = out_dir / f"{cid}_effectcraft_cutout_alpha.webm"
    _render_effectcraft(
        project,
        alpha,
        "render",
        "--format",
        "webm",
        "--channels",
        "rgba",
        required=False,
    )
    video = (
        Path(out_mp4).resolve()
        if out_mp4
        else out_dir / f"{cid}_effectcraft_cutout.mp4"
    )
    _render_effectcraft(project, video, "render", "--format", "h264")
    followed = [
        p["id"]
        for p in rig_plan["parts"]
        if p.get("effectcraft", {}).get("puppet", {}).get("follow")
    ]
    return dict(
        ecproj_path=built["source"],
        ectemplate_path=built["template"],
        values_json_path=str(values),
        secondary_follow_parts=followed,
        webm_alpha_path=str(alpha) if alpha.is_file() else None,
        mp4_path=str(video),
        frame0_path=str(frame0),
        frame1_path=str(frame1),
        mode=mode,
        speed=speed,
        duration=duration,
        mp4_bytes=video.stat().st_size,
    )
