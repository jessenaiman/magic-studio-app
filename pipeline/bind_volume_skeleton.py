"""Bind reconstructed geometry to explicit Z-up rest bones and local animation keys."""
import math


def _parents(bones) -> dict:
    parents = {bone["id"]: bone.get("parent") for bone in bones}
    if not bones or len(parents) != len(bones) or any(not name for name in parents):
        raise ValueError("Skeleton needs unique, nonempty bone IDs")
    return parents


def _check_endpoint(bone, point) -> None:
    if len(point) != 3 or not all(math.isfinite(v) for v in point):
        raise ValueError(f'{bone["id"]}: rest endpoints must be distinct finite 3D points')


def _check_chain(bone, parents) -> None:
    visited, parent = {bone["id"]}, bone.get("parent")
    while parent:
        if parent not in parents or parent in visited:
            raise ValueError(f'{bone["id"]}: missing or cyclic parent {parent}')
        visited.add(parent)
        parent = parents[parent]


def _check_bones(bones, parents) -> None:
    for bone in bones:
        points = [bone["head"], bone["tail"]]
        for point in points:
            _check_endpoint(bone, point)
        if math.dist(*points) <= 1e-8:
            raise ValueError(f'{bone["id"]}: rest endpoints must be distinct finite 3D points')
        _check_chain(bone, parents)


def _check_weight(name, index, weight, count, seen) -> None:
    if not isinstance(index, int) or not 0 <= index < count or index in seen:
        raise ValueError(f"Invalid or repeated vertex in weights for {name}")
    if not math.isfinite(weight) or not 0 < weight <= 1:
        raise ValueError(f"Invalid weight for {name} vertex {index}")


def _vertex_totals(obj, spec, parents) -> list[float]:
    """Weight total per vertex, refusing unknown bones and repeated or invalid entries."""
    totals = [0.0] * len(obj.data.vertices)
    for name, weights in spec["weights"].items():
        if name not in parents:
            raise ValueError(f"Weights refer to missing bone: {name}")
        seen = set()
        for index, weight in weights:
            _check_weight(name, index, weight, len(totals), seen)
            seen.add(index)
            totals[index] += weight
    if any(abs(total - 1) > 1e-5 for total in totals):
        raise ValueError("Every vertex must have explicit weights summing to one")
    return totals


def _check_transform(transform) -> None:
    rotation, location = transform["rotation"], transform.get("location", [0, 0, 0])
    if (
        len(rotation) != 4
        or len(location) != 3
        or not all(math.isfinite(v) for v in rotation + location)
    ):
        raise ValueError("Bone keys need finite WXYZ quaternion and XYZ translation")
    if abs(sum(v * v for v in rotation) - 1) > 1e-5:
        raise ValueError("Bone rotation quaternions must be normalized")


def _check_keys(keys, parents) -> None:
    previous = -1
    for key in keys:
        time = key["time"]
        if not math.isfinite(time) or time < 0 or time <= previous:
            raise ValueError("Animation key times must be finite, nonnegative and increasing")
        previous = time
        if set(key["bones"]) != set(parents):
            raise ValueError("Each animation key must specify every bone")
        for transform in key["bones"].values():
            _check_transform(transform)
    if len(keys) < 2 or keys[0]["time"] != 0:
        raise ValueError("Animation needs at least two keys, starting at zero")


def _armature(bpy, obj, bones, parents):
    """Create the armature, its rest bones and the parent links, and leave object mode on."""
    armature = bpy.data.armatures.new(obj.name + "-skeleton")
    rig = bpy.data.objects.new(obj.name + "-rig", armature)
    bpy.context.scene.collection.objects.link(rig)
    bpy.ops.object.select_all(action="DESELECT")
    rig.select_set(True)
    bpy.context.view_layer.objects.active = rig
    bpy.ops.object.mode_set(mode="EDIT")
    for source in bones:
        bone = armature.edit_bones.new(source["id"])
        bone.head, bone.tail = source["head"], source["tail"]
    for name, parent in parents.items():
        if parent:
            armature.edit_bones[name].parent = armature.edit_bones[parent]
    bpy.ops.object.mode_set(mode="OBJECT")
    return rig


def _vertex_groups(obj, spec):
    """One vertex group per bone, holding that bone's explicit weights."""
    for name, weights in spec["weights"].items():
        group = obj.vertex_groups.new(name=name)
        for index, weight in weights:
            group.add([index], weight, "REPLACE")


def _keyframes(rig, keys, fps) -> None:
    for key in keys:
        frame = 1 + key["time"] * fps
        for name, transform in key["bones"].items():
            bone = rig.pose.bones[name]
            bone.rotation_mode = "QUATERNION"
            bone.rotation_quaternion = transform["rotation"]
            bone.location = transform.get("location", [0, 0, 0])
            bone.keyframe_insert("rotation_quaternion", frame=frame)
            bone.keyframe_insert("location", frame=frame)


def bind(obj, spec, track, fps):
    import bpy

    bones = spec["bones"]
    parents = _parents(bones)
    _check_bones(bones, parents)
    _vertex_totals(obj, spec, parents)
    keys = spec["tracks"][track]
    _check_keys(keys, parents)
    rig = _armature(bpy, obj, bones, parents)
    _vertex_groups(obj, spec)
    obj.parent = rig
    obj.modifiers.new("Skeleton", "ARMATURE").object = rig
    _keyframes(rig, keys, fps)
    return rig
