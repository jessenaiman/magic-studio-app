"""Skeletal cutout construction and export from reviewed canvas-space joints."""

import json
import math
from pathlib import Path
from typing import Any, Dict, Optional


def _skeleton(spec):
    import bpy

    parts = spec["parts"]
    scene = bpy.data.scenes.new(spec["characterId"] + "-animation")
    bpy.context.window.scene = scene
    collection = bpy.data.collections.new(spec["characterId"])
    scene.collection.children.link(collection)
    armature = bpy.data.armatures.new(spec["characterId"] + "-skeleton")
    rig = bpy.data.objects.new(spec["characterId"] + "-rig", armature)
    collection.objects.link(rig)
    bpy.ops.object.select_all(action="DESELECT")
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    for part in parts:
        bone = armature.edit_bones.new(part["id"])
        x, y = part["pivot"]
        tx, ty = part["restTail"]
        bone.head, bone.tail = (x, -y, 0), (tx, -ty, 0)
    for part in parts:
        if part["parent"]:
            armature.edit_bones[part["id"]].parent = armature.edit_bones[part["parent"]]
    bpy.ops.object.mode_set(mode="OBJECT")
    return scene, collection, rig


def _artwork(collection, rig, part, index, canvas):
    import bpy

    width, height = canvas
    image = bpy.data.images.load(part["fullLayerPath"], check_existing=True)
    if tuple(image.size) != (width, height):
        raise ValueError(f"{part['id']}: full layer must match the rest canvas")
    mesh = bpy.data.meshes.new(part["id"])
    depth = index * 0.01
    mesh.from_pydata(
        [
            (0, -height, depth),
            (width, -height, depth),
            (width, 0, depth),
            (0, 0, depth),
        ],
        [],
        [(0, 1, 2, 3)],
    )
    mesh.uv_layers.new()
    for loop, uv in zip(
        mesh.uv_layers.active.data, [(0, 0), (1, 0), (1, 1), (0, 1)], strict=True
    ):
        loop.uv = uv
    obj = bpy.data.objects.new(part["id"] + "-artwork", mesh)
    collection.objects.link(obj)
    obj.parent = rig
    obj.vertex_groups.new(name=part["id"]).add(list(range(4)), 1.0, "REPLACE")
    obj.modifiers.new("Skeleton", "ARMATURE").object = rig
    material = bpy.data.materials.new(part["id"] + "-material")
    material.use_nodes = True
    nodes, links = material.node_tree.nodes, material.node_tree.links
    shader = next(n for n in nodes if n.type == "BSDF_PRINCIPLED")
    texture = nodes.new("ShaderNodeTexImage")
    texture.image = image
    links.new(texture.outputs["Color"], shader.inputs["Base Color"])
    links.new(texture.outputs["Alpha"], shader.inputs["Alpha"])
    links.new(texture.outputs["Color"], shader.inputs["Emission Color"])
    shader.inputs["Emission Strength"].default_value = 1
    shader.inputs["Roughness"].default_value = 1
    material.surface_render_method = "DITHERED"
    mesh.materials.append(material)
    image.pack()


def _channel(left, right, part, amount, key):
    start = left.get(part, {}).get(key, 0)
    return start + (right.get(part, {}).get(key, 0) - start) * amount


def _animate(scene, rig, spec):
    from mathutils import Quaternion, Vector

    parts = spec["parts"]
    scene.render.fps = 24
    scene.frame_start = 1
    scene.frame_end = max(1, math.ceil(spec["defaultDuration"] * scene.render.fps))
    beats = spec["beats"]
    period = beats[-1][0]
    for frame in range(scene.frame_start, scene.frame_end + 2):
        time = (frame - 1) / scene.render.fps
        time = time % period if spec["loop"] else min(time, period)
        left, right = next(
            (
                (a, b)
                for a, b in zip(beats, beats[1:], strict=False)
                if a[0] <= time <= b[0]
            ),
            (beats[-1], beats[-1]),
        )
        u = (time - left[0]) / (right[0] - left[0]) if right[0] > left[0] else 0
        u = u * u * (3 - 2 * u)
        a, b = spec["poses"][left[1]]["parts"], spec["poses"][right[1]]["parts"]
        for part in parts:
            bone = rig.pose.bones[part["id"]]

            low, high = part["rotationLimitsDeg"]
            angle = -math.radians(
                max(low, min(high, _channel(a, b, part["id"], u, "rotation")))
            )
            rest = bone.bone.matrix_local.to_quaternion()
            bone.rotation_mode = "QUATERNION"
            bone.rotation_quaternion = (
                rest.inverted() @ Quaternion((0, 0, 1), angle) @ rest
            )
            bone.location = rest.inverted() @ Vector(
                (
                    _channel(a, b, part["id"], u, "x"),
                    -_channel(a, b, part["id"], u, "y"),
                    0,
                )
            )
            bone.keyframe_insert("rotation_quaternion", frame=frame)
            bone.keyframe_insert("location", frame=frame)
    rig["source_spec"] = json.dumps(spec)
    rig["representation"] = "skeletal_cutout_2d"


def _camera(scene, collection, canvas):
    import bpy
    from mathutils import Vector

    width, height = canvas
    bounds = []
    for frame in range(scene.frame_start, scene.frame_end + 1):
        scene.frame_set(frame)
        graph = bpy.context.evaluated_depsgraph_get()
        for obj in collection.objects:
            if obj.type == "MESH":
                evaluated = obj.evaluated_get(graph)
                bounds.extend(
                    evaluated.matrix_world @ Vector(corner)
                    for corner in evaluated.bound_box
                )
    left, right = min(v.x for v in bounds), max(v.x for v in bounds)
    bottom, top = min(v.y for v in bounds), max(v.y for v in bounds)
    camera = bpy.data.cameras.new("Cutout camera")
    camera.type = "ORTHO"
    camera.ortho_scale = max(right - left, (top - bottom) * width / height) * 1.02
    camera.sensor_fit = "HORIZONTAL"
    camera.clip_end = max(width, height) * 4
    view = bpy.data.objects.new("Cutout camera", camera)
    collection.objects.link(view)
    view.location = ((left + right) / 2, (bottom + top) / 2, max(width, height))
    scene.camera = view
    scene.render.resolution_x, scene.render.resolution_y = width, height
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.view_settings.view_transform = "Standard"
    scene.frame_set(1)


def build(spec, blend_path, glb_path=None):
    import bpy

    scene, collection, rig = _skeleton(spec)
    for index, part in enumerate(spec["parts"]):
        _artwork(collection, rig, part, index, spec["canvasSize"])
    _animate(scene, rig, spec)
    _camera(scene, collection, spec["canvasSize"])
    Path(blend_path).parent.mkdir(parents=True, exist_ok=True)
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path))
    if glb_path:
        Path(glb_path).parent.mkdir(parents=True, exist_ok=True)
        bpy.ops.object.select_all(action="DESELECT")
        for obj in collection.objects:
            if obj.type in {"MESH", "ARMATURE"}:
                obj.select_set(True)
        bpy.ops.export_scene.gltf(
            filepath=str(glb_path),
            export_format="GLB",
            use_selection=True,
            use_active_scene=True,
        )


def _bone_tail(rig_plan, part):
    tail = rig_plan.get("joints", {}).get(part["id"], {}).get("rest_tail")
    if not isinstance(tail, (list, tuple)) or len(tail) != 2:
        raise ValueError(
            f"{part['id']}: joints.rest_tail requires a reviewed "
            "canvas-space bone endpoint"
        )
    tail = [float(v) for v in tail]
    if (
        not all(math.isfinite(v) for v in tail + part["pivot"])
        or math.dist(tail, part["pivot"]) < 0.001
    ):
        raise ValueError(f"{part['id']}: bone endpoints must be finite and distinct")
    part["restTail"] = tail


def _reviewed_parts(rig_plan, parts):
    ids = {part["id"] for part in parts}
    if not ids or len(ids) != len(parts):
        raise ValueError("Blender rig requires distinct, nonempty part IDs")
    parents = {part["id"]: part["parent"] for part in parts}
    for part in parts:
        _bone_tail(rig_plan, part)
        visited, parent = {part["id"]}, part["parent"]
        while parent:
            if parent not in ids or parent in visited:
                raise ValueError(f"{part['id']}: missing or cyclic parent {parent}")
            visited.add(parent)
            parent = parents[parent]
        part["fullLayerPath"] = str(Path(part["fullLayerPath"]).resolve(strict=True))
    return parts


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
    from pipeline.rig_motion import pose_beats

    spec = dict(
        characterId=rig_plan["character_id"],
        canvasSize=rig_plan["canvas_size"],
        defaultDuration=duration,
        poses=pose_library.get("poses", {}),
        parts=[
            dict(
                id=p["id"],
                parent=p.get("parent"),
                pivot=p["pivot"],
                rotationLimitsDeg=p["rotation"],
                fullLayerPath=p["full_layer_path"],
            )
            for p in sorted(rig_plan["parts"], key=lambda p: p["layer"])
        ],
    )
    if (
        not math.isfinite(speed)
        or speed <= 0
        or not math.isfinite(duration)
        or duration <= 0
    ):
        raise ValueError(
            "Blender animation speed and duration must be positive and finite"
        )
    spec["parts"] = _reviewed_parts(rig_plan, spec["parts"])
    spec["loop"] = bool(pose_library.get("loop", False))
    spec["beats"] = pose_beats(pose_library, mode, speed)
    spec["sourceImage"] = rig_plan.get("source_image")
    spec["provenance"] = rig_plan.get("provenance", {})
    runtime = Path(__file__).read_text()
    out_py_path.parent.mkdir(parents=True, exist_ok=True)
    blend = str(Path(blend_path or out_py_path.with_suffix(".blend")).resolve())
    glb = str(Path(glb_path).resolve()) if glb_path else None
    out_py_path.write_text(
        runtime
        + f"\nRIG_SPEC = json.loads({json.dumps(json.dumps(spec))})\n"
        + f"build(RIG_SPEC, {blend!r}, {glb!r})\n",
        encoding="utf-8",
    )
    return out_py_path
