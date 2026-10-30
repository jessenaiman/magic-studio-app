"""Stage 3C: Hierarchical cutout animation, Essential Graphics .ectemplate & secondary puppet motion via effectcraft-cli."""

from __future__ import annotations

import json
import re
import subprocess
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


def build_pose_library(character_id: str) -> Dict[str, Any]:
    """Pose-library-design timing & joint rotations."""
    return {
        "character_id": character_id,
        "acting_pattern": "anticipation -> action -> hold -> settle",
        "poses": {
            "idle": {
                "pose": "idle",
                "category": "Neutral",
                "hold_frames": 24,
                "transition": "sine.inOut",
                "parts": {
                    "torso": {"rotation": 0.0, "y": 0.0},
                    "head": {"rotation": 1.5},
                    "ear_left": {"rotation": -4.0},
                    "ear_right": {"rotation": 4.0},
                    "arm_left": {"rotation": 4.0},
                    "arm_right": {"rotation": -2.0},
                    "leg_left": {"rotation": 2.0},
                    "foot_left": {"rotation": -8.0},
                    "leg_right": {"rotation": -2.0},
                    "foot_right": {"rotation": 6.0},
                },
            },
            "idle_breathe": {
                "pose": "idle_breathe",
                "category": "Neutral",
                "hold_frames": 12,
                "transition": "sine.inOut",
                "parts": {
                    "torso": {"rotation": -1.5, "y": -3.0},
                    "head": {"rotation": -2.0},
                    "ear_left": {"rotation": 5.0},
                    "ear_right": {"rotation": -5.0},
                    "arm_left": {"rotation": 10.0},
                    "arm_right": {"rotation": 3.0},
                    "leg_left": {"rotation": -3.0},
                    "foot_left": {"rotation": 12.0},
                    "leg_right": {"rotation": 2.0},
                    "foot_right": {"rotation": -10.0},
                },
            },
            "walk_contact": {
                "pose": "walk_contact",
                "category": "Action",
                "hold_frames": 6,
                "transition": "power2.inOut",
                "parts": {
                    "torso": {"rotation": 3.0, "y": -5.0},
                    "head": {"rotation": -2.5},
                    "ear_left": {"rotation": -10.0},
                    "ear_right": {"rotation": -6.0},
                    "arm_left": {"rotation": 18.0},
                    "arm_right": {"rotation": -4.0},
                    "leg_left": {"rotation": -12.0},
                    "foot_left": {"rotation": -16.0},
                    "leg_right": {"rotation": 12.0},
                    "foot_right": {"rotation": 16.0},
                },
            },
            "walk_passing": {
                "pose": "walk_passing",
                "category": "Action",
                "hold_frames": 6,
                "transition": "power2.inOut",
                "parts": {
                    "torso": {"rotation": -2.5, "y": 4.0},
                    "head": {"rotation": 2.5},
                    "ear_left": {"rotation": 8.0},
                    "ear_right": {"rotation": 10.0},
                    "arm_left": {"rotation": -14.0},
                    "arm_right": {"rotation": 4.0},
                    "leg_left": {"rotation": 12.0},
                    "foot_left": {"rotation": 16.0},
                    "leg_right": {"rotation": -12.0},
                    "foot_right": {"rotation": -16.0},
                },
            },
            "run_contact": {
                "pose": "run_contact",
                "category": "Action",
                "hold_frames": 4,
                "transition": "power2.inOut",
                "parts": {
                    "torso": {"rotation": 5.0, "y": -10.0},
                    "head": {"rotation": -4.0},
                    "ear_left": {"rotation": -16.0},
                    "ear_right": {"rotation": -14.0},
                    "arm_left": {"rotation": 28.0},
                    "arm_right": {"rotation": -6.0},
                    "leg_left": {"rotation": -18.0},
                    "foot_left": {"rotation": -20.0},
                    "leg_right": {"rotation": 18.0},
                    "foot_right": {"rotation": 20.0},
                },
            },
            "run_passing": {
                "pose": "run_passing",
                "category": "Action",
                "hold_frames": 4,
                "transition": "power2.inOut",
                "parts": {
                    "torso": {"rotation": -4.0, "y": 6.0},
                    "head": {"rotation": 4.0},
                    "ear_left": {"rotation": 14.0},
                    "ear_right": {"rotation": 16.0},
                    "arm_left": {"rotation": -22.0},
                    "arm_right": {"rotation": 6.0},
                    "leg_left": {"rotation": 18.0},
                    "foot_left": {"rotation": 20.0},
                    "leg_right": {"rotation": -18.0},
                    "foot_right": {"rotation": -20.0},
                },
            },
            "jump_anticipation": {
                "pose": "jump_anticipation",
                "category": "Action",
                "hold_frames": 8,
                "transition": "power2.in",
                "parts": {
                    "torso": {"rotation": 5.0, "y": 14.0},
                    "head": {"rotation": -6.0},
                    "ear_left": {"rotation": -12.0},
                    "ear_right": {"rotation": -10.0},
                    "arm_left": {"rotation": -18.0},
                    "arm_right": {"rotation": 5.0},
                    "leg_left": {"rotation": 14.0},
                    "foot_left": {"rotation": 14.0},
                    "leg_right": {"rotation": 12.0},
                    "foot_right": {"rotation": 14.0},
                },
            },
            "jump_apex": {
                "pose": "jump_apex",
                "category": "Action",
                "hold_frames": 14,
                "transition": "back.out",
                "parts": {
                    "torso": {"rotation": -4.0, "y": -32.0},
                    "head": {"rotation": 6.0},
                    "ear_left": {"rotation": 14.0},
                    "ear_right": {"rotation": 16.0},
                    "arm_left": {"rotation": 28.0},
                    "arm_right": {"rotation": -6.0},
                    "leg_left": {"rotation": -16.0},
                    "foot_left": {"rotation": -18.0},
                    "leg_right": {"rotation": -14.0},
                    "foot_right": {"rotation": -16.0},
                },
            },
            "wave_greeting": {
                "pose": "wave_greeting",
                "category": "Emotion",
                "hold_frames": 18,
                "transition": "back.out",
                "parts": {
                    "torso": {"rotation": -2.0, "y": 0.0},
                    "head": {"rotation": -5.0},
                    "ear_left": {"rotation": -10.0},
                    "ear_right": {"rotation": 8.0},
                    "arm_left": {"rotation": 32.0},
                    "arm_right": {"rotation": 4.0},
                    "leg_left": {"rotation": -8.0},
                    "foot_left": {"rotation": -14.0},
                    "leg_right": {"rotation": 6.0},
                    "foot_right": {"rotation": 12.0},
                },
            },
            "wave_greeting_alt": {
                "pose": "wave_greeting_alt",
                "category": "Emotion",
                "hold_frames": 12,
                "transition": "sine.inOut",
                "parts": {
                    "torso": {"rotation": 2.0, "y": 0.0},
                    "head": {"rotation": 4.0},
                    "ear_left": {"rotation": 8.0},
                    "ear_right": {"rotation": -8.0},
                    "arm_left": {"rotation": -12.0},
                    "arm_right": {"rotation": -3.0},
                    "leg_left": {"rotation": 6.0},
                    "foot_left": {"rotation": 12.0},
                    "leg_right": {"rotation": -6.0},
                    "foot_right": {"rotation": -12.0},
                },
            },
        },
        "sequences": {
            "idle": ["idle", "idle_breathe", "idle"],
            "walk": ["walk_contact", "walk_passing", "walk_contact", "walk_passing"],
            "run": ["run_contact", "run_passing", "run_contact", "run_passing"],
            "jump": ["idle", "jump_anticipation", "jump_apex", "idle"],
            "wave": [
                "idle",
                "wave_greeting",
                "wave_greeting_alt",
                "wave_greeting",
                "idle",
            ],
            "static": ["idle", "idle"],
        },
    }


def _mode_beat_times(mode: str, speed: float = 1.0) -> List[Tuple[float, str]]:
    m = (mode or "walk").lower().strip()
    inv_speed = 1.0 / max(0.1, float(speed))
    table = {
        "run": [
            (0.0, "idle"),
            (0.18, "run_contact"),
            (0.32, "run_contact"),
            (0.55, "run_passing"),
            (0.70, "run_passing"),
            (0.95, "idle"),
        ],
        "jump": [
            (0.0, "idle"),
            (0.25, "jump_anticipation"),
            (0.45, "jump_anticipation"),
            (0.75, "jump_apex"),
            (1.10, "jump_apex"),
            (1.50, "idle"),
        ],
        "wave": [
            (0.0, "idle"),
            (0.25, "wave_greeting"),
            (0.60, "wave_greeting_alt"),
            (0.95, "wave_greeting"),
            (1.25, "wave_greeting"),
            (1.50, "idle"),
        ],
        "idle": [
            (0.0, "idle"),
            (0.60, "idle_breathe"),
            (0.90, "idle_breathe"),
            (1.50, "idle"),
        ],
        "static": [(0.0, "idle"), (1.50, "idle")],
    }
    raw = table.get(
        m,
        [
            (0.0, "idle"),
            (0.25, "walk_contact"),
            (0.50, "walk_contact"),
            (0.85, "walk_passing"),
            (1.10, "walk_passing"),
            (1.50, "idle"),
        ],
    )
    return [(round(t * inv_speed, 4), pose) for t, pose in raw]


def export_effectcraft_jsx_script(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_jsx_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
) -> Path:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    comp_w, comp_h = int(w) + (int(w) % 2), int(h) + (int(h) % 2)
    comp_dur = max(0.5, float(duration))
    parts_sorted = sorted(rig_plan["parts"], key=lambda p: p["layer"])
    beat_times = _mode_beat_times(mode, speed)
    poses = pose_library.get("poses", {})

    lines = [
        f"// EffectCraft Cutout Rig Script for '{cid}' ({mode})",
        f"var comp = app.project.items.addComp({json.dumps(f'{cid}_{mode}_cutout')}, {comp_w}, {comp_h}, 1.0, {comp_dur:.3f}, 24);",
        "var layerMap = {};",
    ]
    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        px, py = float(part["pivot"][0]), float(part["pivot"][1])
        lines.extend(
            [
                f"var item_{v_pid} = app.project.importFile(new ImportOptions(new File({json.dumps(part['full_layer_path'])})));",
                f"var lyr_{v_pid} = comp.layers.add(item_{v_pid}); lyr_{v_pid}.name = {json.dumps(pid)};",
                f"lyr_{v_pid}.property('Anchor Point').setValue([{px:.1f}, {py:.1f}, 0.0]);",
                f"lyr_{v_pid}.property('Position').setValue([{px:.1f}, {py:.1f}, 0.0]);",
                f"layerMap[{json.dumps(pid)}] = lyr_{v_pid};",
            ]
        )
    for part in parts_sorted:
        if part.get("parent"):
            lines.append(
                f"if (layerMap[{json.dumps(part['parent'])}]) layerMap[{json.dumps(part['id'])}].parent = layerMap[{json.dumps(part['parent'])}];"
            )
    for part in parts_sorted:
        pid = part["id"]
        v_pid = re.sub(r"\W", "_", pid)
        r_min, r_max = part["rotation"]
        lines.append(
            f"var rot_{v_pid} = layerMap[{json.dumps(pid)}].property('Rotation');"
        )
        for t_sec, pose_name in beat_times:
            rot_val = (
                0.0
                if mode == "static"
                else float(
                    poses.get(pose_name, {})
                    .get("parts", {})
                    .get(pid, {})
                    .get("rotation", 0.0)
                )
            )
            lines.append(
                f"rot_{v_pid}.setValueAtTime({t_sec:.4f}, {max(float(r_min), min(float(r_max), rot_val)):.2f});"
            )
        lines.append(f"rot_{v_pid}.expression = 'loopOut(\"cycle\")';")

    out_jsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_jsx_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return out_jsx_path


def _native_cutout_scene(rig_plan, pose_library, mode, speed, duration):
    layers, controls = [], []
    beats = _mode_beat_times(mode, speed)
    for part in sorted(rig_plan["parts"], key=lambda p: p["layer"]):
        pid, pivot = part["id"], [*part["pivot"], 0.0]
        low, high = part["rotation"]
        keys = []
        for time, pose in beats:
            rotation = (
                pose_library["poses"]
                .get(pose, {})
                .get("parts", {})
                .get(pid, {})
                .get("rotation", 0.0)
            )
            keys.append(
                [
                    time,
                    0.0 if mode == "static" else max(low, min(high, float(rotation))),
                ]
            )
        authored = part.get("effectcraft", {})
        properties = {
            "transform/rotation": {"keys": keys, "expression": 'loopOut("cycle")'}
        }
        properties.update(authored.get("properties", {}))
        layer = dict(
            id=pid,
            file=str(Path(part["full_layer_path"]).resolve()),
            anchor=pivot,
            position=pivot,
            properties=properties,
        )
        if part.get("parent"):
            layer["parent"] = part["parent"]
        layer.update({k: authored[k] for k in ("puppet", "sketch") if k in authored})
        layers.append(layer)
        controls.extend(
            [
                dict(layer=pid, path="transform/rotation", name=f"{pid}_rotation"),
                dict(layer=pid, media=True, name=f"{pid}_media"),
            ]
        )
    canvas = [int(v) + int(v) % 2 for v in rig_plan["canvas_size"]]
    return dict(
        name=f"{rig_plan['character_id']}_cutout_rig",
        canvas=canvas,
        fps=24,
        duration=duration,
        layers=layers,
        controls=controls,
    )


def _render_effectcraft(project, output, *options, required=True):
    output.parent.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
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
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    out_mp4: Optional[Path] = None,
    out_jsx: Optional[Path] = None,
) -> Dict[str, Any]:
    from time import time_ns

    try:
        from .effectcraft_scene import SceneBuilder
        from .effectcraft_project import NativeProject
    except ImportError:
        from effectcraft_scene import SceneBuilder
        from effectcraft_project import NativeProject

    cid = rig_plan["character_id"]
    out_dir = Path(out_dir).resolve()
    out_dir.mkdir(parents=True, exist_ok=True)
    duration = max(0.5, float(duration))
    spec = _native_cutout_scene(rig_plan, pose_library, mode, speed, duration)
    source = out_dir / f"{cid}_scene.json"
    source.write_text(json.dumps(spec, indent=2) + "\n")
    built = SceneBuilder(source, out_dir / "native" / str(time_ns())).build()
    project = NativeProject(built["source"])
    jsx = Path(out_jsx).resolve() if out_jsx else out_dir / f"{cid}_cutout_rig.jsx"
    export_effectcraft_jsx_script(
        rig_plan, pose_library, jsx, mode=mode, speed=speed, duration=duration
    )
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
    beats = _mode_beat_times(mode, speed)
    peak = min(duration * 0.5, beats[1][0] if len(beats) > 1 else 0.35)
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
        jsx_path=str(jsx),
        webm_alpha_path=str(alpha) if alpha.is_file() else None,
        mp4_path=str(video),
        frame0_path=str(frame0),
        frame1_path=str(frame1),
        mode=mode,
        speed=speed,
        duration=duration,
        mp4_bytes=video.stat().st_size,
    )


def render_ectemplate_with_values(
    template_path: Path,
    values_path: Path,
    out_path: Path,
    fmt: str = "h264",
    channels: str = "rgb",
) -> Dict[str, Any]:
    """Render a reusable .ectemplate with a values.json override file via effectcraft-cli."""
    try:
        from .effectcraft_project import effectcraft_cli
    except ImportError:
        from effectcraft_project import effectcraft_cli
    ec_bin = effectcraft_cli()
    out_path = Path(out_path).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)
    cmd = [
        ec_bin,
        "render",
        "--template",
        str(Path(template_path).resolve()),
        "--values",
        str(Path(values_path).resolve()),
        "--out",
        str(out_path),
        "--format",
        fmt,
        "--channels",
        channels,
    ]
    proc = subprocess.run(cmd, capture_output=True, text=True, timeout=90)
    if proc.returncode != 0:
        raise RuntimeError(
            f"effectcraft-cli template render failed: {proc.stderr or proc.stdout}"
        )
    return {
        "ok": out_path.is_file(),
        "template": str(template_path),
        "values": str(values_path),
        "out": str(out_path),
        "bytes": out_path.stat().st_size if out_path.is_file() else 0,
    }


def _build_serializable_rig_spec(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Dict[str, Any]:
    cid = rig_plan["character_id"]
    w, h = rig_plan["canvas_size"]
    return {
        "schemaVersion": "2.0",
        "characterId": cid,
        "canvasSize": [int(w), int(h)],
        "defaultMode": mode,
        "defaultSpeed": float(speed),
        "defaultDuration": float(duration),
        "glbPath": glb_path or f"exports/blender_ingest/{cid}/{cid}_4dir.glb",
        "parts": [
            {
                "id": p["id"],
                "layer": int(p["layer"]),
                "parent": p.get("parent"),
                "pivot": [float(p["pivot"][0]), float(p["pivot"][1])],
                "rotationLimitsDeg": [float(p["rotation"][0]), float(p["rotation"][1])],
                "fullLayerPath": p["full_layer_path"],
                "croppedLayerPath": p.get("cropped_path", p["full_layer_path"]),
            }
            for p in sorted(rig_plan["parts"], key=lambda p: p["layer"])
        ],
        "poses": pose_library.get("poses", {}),
        "modeBeats": {
            m: _mode_beat_times(m, 1.0)
            for m in ("idle", "walk", "run", "jump", "wave", "static")
        },
    }


def export_threejs_bundle(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_js_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Path:
    spec = _build_serializable_rig_spec(
        rig_plan, pose_library, mode, speed, duration, glb_path
    )
    out_js_path.parent.mkdir(parents=True, exist_ok=True)
    out_js_path.write_text(
        f"export const RIG_SPEC = {json.dumps(spec, indent=2)};\n", encoding="utf-8"
    )
    return out_js_path


def export_react_motion_component(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_tsx_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
) -> Path:
    spec = _build_serializable_rig_spec(
        rig_plan, pose_library, mode, speed, duration, glb_path
    )
    out_tsx_path.parent.mkdir(parents=True, exist_ok=True)
    out_tsx_path.write_text(
        f"export const RIG_CONFIG = {json.dumps(spec, indent=2)} as const;\n",
        encoding="utf-8",
    )
    return out_tsx_path


def export_blender_python_script(
    rig_plan: Dict[str, Any],
    pose_library: Dict[str, Any],
    out_py_path: Path,
    mode: str = "walk",
    speed: float = 1.0,
    duration: float = 3.0,
    glb_path: Optional[str] = None,
    blend_path: Optional[str] = None,
) -> Path:
    spec = _build_serializable_rig_spec(
        rig_plan, pose_library, mode, speed, duration, glb_path
    )
    import math

    ids = {part["id"] for part in spec["parts"]}
    if not ids or len(ids) != len(spec["parts"]):
        raise ValueError("Blender rig requires distinct, nonempty part IDs")
    parents = {part["id"]: part["parent"] for part in spec["parts"]}
    for part in spec["parts"]:
        tail = rig_plan.get("joints", {}).get(part["id"], {}).get("rest_tail")
        if not isinstance(tail, (list, tuple)) or len(tail) != 2:
            raise ValueError(
                f"{part['id']}: joints.rest_tail must define the reviewed bone endpoint in canvas pixels"
            )
        tail = [float(v) for v in tail]
        if (
            not all(math.isfinite(v) for v in tail + part["pivot"])
            or math.dist(tail, part["pivot"]) < 0.001
        ):
            raise ValueError(
                f"{part['id']}: bone endpoints must be finite and distinct"
            )
        part["restTail"] = tail
        visited, parent = {part["id"]}, part["parent"]
        while parent:
            if parent not in ids or parent in visited:
                raise ValueError(f"{part['id']}: missing or cyclic parent {parent}")
            visited.add(parent)
            parent = parents[parent]
        part["fullLayerPath"] = str(Path(part["fullLayerPath"]).resolve(strict=True))
    if (
        not math.isfinite(speed)
        or speed <= 0
        or not math.isfinite(duration)
        or duration <= 0
    ):
        raise ValueError(
            "Blender animation speed and duration must be positive and finite"
        )
    sequence = pose_library.get("sequences", {}).get(mode)
    if not isinstance(sequence, list) or not sequence:
        raise ValueError(f"Missing authored animation sequence: {mode}")
    spec["loop"] = bool(
        pose_library.get("loop", mode in {"idle", "walk", "run", "sway"})
    )
    spec["beats"], cursor = [], 0.0
    for pose in sequence:
        if pose not in spec["poses"]:
            raise ValueError(f"Missing animation pose: {pose}")
        hold = float(spec["poses"][pose].get("hold_frames", 1))
        if not math.isfinite(hold) or hold <= 0:
            raise ValueError(f"Invalid hold_frames for pose: {pose}")
        spec["beats"].append([cursor, pose])
        cursor += hold / (24 * speed)
    spec["beats"].append([cursor, sequence[0] if spec["loop"] else sequence[-1]])
    spec["sourceImage"] = rig_plan.get("source_image")
    spec["provenance"] = rig_plan.get("provenance", {})
    runtime = Path(__file__).with_name("blender_cutout_rig.py").read_text()
    out_py_path.parent.mkdir(parents=True, exist_ok=True)
    out_py_path.write_text(
        runtime
        + f"\nRIG_SPEC = json.loads({json.dumps(json.dumps(spec))})\n"
        + f"build(RIG_SPEC, {str(Path(blend_path or out_py_path.with_suffix('.blend')).resolve())!r}, {str(Path(glb_path).resolve()) if glb_path else None!r})\n",
        encoding="utf-8",
    )
    return out_py_path
