"""Bind reconstructed geometry to explicit Z-up rest bones and local animation keys."""
import math


def bind(obj, spec, track, fps):
    import bpy

    bones = spec["bones"]
    parents = {bone["id"]: bone.get("parent") for bone in bones}
    if not bones or len(parents) != len(bones) or any(not name for name in parents):
        raise ValueError("Skeleton needs unique, nonempty bone IDs")
    for bone in bones:
        points = [bone["head"], bone["tail"]]
        if any(len(p) != 3 or not all(math.isfinite(v) for v in p) for p in points) or math.dist(*points) <= 1e-8:
            raise ValueError(f'{bone["id"]}: rest endpoints must be distinct finite 3D points')
        visited, parent = {bone["id"]}, bone.get("parent")
        while parent:
            if parent not in parents or parent in visited:
                raise ValueError(f'{bone["id"]}: missing or cyclic parent {parent}')
            visited.add(parent)
            parent = parents[parent]
    totals = [0.0] * len(obj.data.vertices)
    for name, weights in spec["weights"].items():
        if name not in parents:
            raise ValueError(f"Weights refer to missing bone: {name}")
        seen = set()
        for index, weight in weights:
            if not isinstance(index, int) or not 0 <= index < len(totals) or index in seen:
                raise ValueError(f"Invalid or repeated vertex in weights for {name}")
            if not math.isfinite(weight) or not 0 < weight <= 1:
                raise ValueError(f"Invalid weight for {name} vertex {index}")
            seen.add(index)
            totals[index] += weight
    if any(abs(total - 1) > 1e-5 for total in totals):
        raise ValueError("Every vertex must have explicit weights summing to one")
    keys = spec["tracks"][track]
    previous = -1
    for key in keys:
        time = key["time"]
        if not math.isfinite(time) or time < 0 or time <= previous:
            raise ValueError("Animation key times must be finite, nonnegative and increasing")
        previous = time
        if set(key["bones"]) != set(parents):
            raise ValueError("Each animation key must specify every bone")
        for transform in key["bones"].values():
            rotation, location = transform["rotation"], transform.get("location", [0, 0, 0])
            if len(rotation) != 4 or len(location) != 3 or not all(math.isfinite(v) for v in rotation + location):
                raise ValueError("Bone keys need finite WXYZ quaternion and XYZ translation")
            if abs(sum(v*v for v in rotation) - 1) > 1e-5:
                raise ValueError("Bone rotation quaternions must be normalized")
    if len(keys) < 2 or keys[0]["time"] != 0:
        raise ValueError("Animation needs at least two keys, starting at zero")
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
    for name, weights in spec["weights"].items():
        group = obj.vertex_groups.new(name=name)
        for index, weight in weights:
            group.add([index], weight, "REPLACE")
    obj.parent = rig
    obj.modifiers.new("Skeleton", "ARMATURE").object = rig
    for key in keys:
        for name, transform in key["bones"].items():
            bone = rig.pose.bones[name]
            bone.rotation_mode = "QUATERNION"
            bone.rotation_quaternion = transform["rotation"]
            bone.location = transform.get("location", [0, 0, 0])
            bone.keyframe_insert("rotation_quaternion", frame=1 + key["time"] * fps)
            bone.keyframe_insert("location", frame=1 + key["time"] * fps)
    return rig
