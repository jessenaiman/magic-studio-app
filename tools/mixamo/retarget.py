"""Blender 5.3: bake Mixamo motions using aligned world-space rotation deltas.

Usage: blender-5.3 -b --factory-startup --python-exit-code 1 --python retarget.py -- SOURCE OUT.glb
Keeps the character's original actions. No source mesh is modified.
"""
import bpy
import math
import re
import sys
from pathlib import Path
from mathutils import Quaternion, Vector

ROOT = Path(__file__).resolve().parents[2]
src, output = map(Path, sys.argv[sys.argv.index('--') + 1:])
bpy.ops.object.select_all(action='SELECT')
bpy.ops.object.delete(use_global=False)
bpy.context.scene.render.fps = 30
for action in list(bpy.data.actions):
    bpy.data.actions.remove(action)


def load(path):
    before = set(bpy.data.objects)
    if path.suffix.lower() == '.fbx':
        bpy.ops.import_scene.fbx(filepath=str(path), automatic_bone_orientation=False)
    else:
        bpy.ops.import_scene.gltf(filepath=str(path))
    return [o for o in bpy.data.objects if o not in before]


def curves(action):
    if hasattr(action, 'fcurves'):
        return list(action.fcurves)
    return [fc for layer in action.layers for strip in layer.strips
            for bag in strip.channelbags for fc in bag.fcurves]


def left_angle(arm):
    v = arm.matrix_world.to_3x3() @ (arm.data.bones['mixamorig:LeftArm'].head_local - arm.data.bones['mixamorig:RightArm'].head_local)
    return math.atan2(v.y, v.x)


objects = load(src)
arm = next(o for o in objects if o.type == 'ARMATURE')
arm.animation_data_create()
own = list(bpy.data.actions)
for action in own:
    action.name = re.sub(r'\.\d{3}$', '', action.name.split('|')[-1])
for track in list(arm.animation_data.nla_tracks):
    arm.animation_data.nla_tracks.remove(track)
arm.animation_data.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
    pb.rotation_mode = 'QUATERNION'
bpy.context.view_layer.update()
target_world = arm.matrix_world.copy()
world_rotation_inverse = target_world.to_quaternion().inverted()
root_name = 'mixamorig:Hips'
assert arm.data.bones[root_name].parent is None
target_hip = target_world @ arm.data.bones[root_name].head_local
target_rest = {b.name: (target_world @ b.matrix_local).to_quaternion() for b in arm.data.bones}
rest_local = {b.name: ((b.parent.matrix_local.inverted() @ b.matrix_local) if b.parent else b.matrix_local).to_quaternion()
              for b in arm.data.bones}
clips = list(own)

for path in sorted((ROOT / 'assets/animations/mixamo').glob('*.fbx')):
    imported = load(path)
    source = next(o for o in imported if o.type == 'ARMATURE')
    source_action = source.animation_data.action
    assert bpy.context.scene.render.fps == 30
    names = [b.name for b in arm.data.bones if b.name in source.data.bones]
    assert len(names) == len(arm.data.bones), (path, len(names))
    align = Quaternion(Vector((0, 0, 1)), left_angle(arm) - left_angle(source))
    source_rest = {name: (source.matrix_world @ source.data.bones[name].matrix_local).to_quaternion() for name in names}
    # A target can use Mixamo names while being bound in an A-pose. Copying
    # source T-pose deltas onto that rest pose folds the arms into the torso.
    def child_name(name):
        if name.endswith('Hand'):return name+'Middle1'
        children=[b.name for b in arm.data.bones[name].children if b.name in source.data.bones]
        return children[0] if children else None
    deviations=[]
    for side in ('Left','Right'):
        for part in ('Arm','ForeArm'):
            name='mixamorig:'+side+part;child=child_name(name)
            td=target_world.to_3x3()@(arm.data.bones[child].head_local-arm.data.bones[name].head_local)
            sd=source.matrix_world.to_3x3()@(source.data.bones[child].head_local-source.data.bones[name].head_local)
            deviations.append(math.degrees(td.angle(align@sd)))
    canonical_arms=max(deviations)>20
    aligned_rest={n:q.copy() for n,q in target_rest.items()}
    if canonical_arms:
        for name in names:
            if not any(name.startswith('mixamorig:'+s+n) for s in ('Left','Right') for n in ('Shoulder','Arm','ForeArm','Hand')):continue
            child=child_name(name)
            if child:
                td=target_world.to_3x3()@(arm.data.bones[child].head_local-arm.data.bones[name].head_local)
                sd=source.matrix_world.to_3x3()@(source.data.bones[child].head_local-source.data.bones[name].head_local)
                if td.length>1e-6 and sd.length>1e-6:aligned_rest[name]=td.rotation_difference(align@sd)@target_rest[name]
        print('CANONICAL_ARM_REST',max(deviations),flush=True)
    source_hip = source.matrix_world @ source.data.bones[root_name].head_local
    ratio = target_hip.z / source_hip.z
    assert ratio > 0
    root_delta_basis = arm.data.bones[root_name].matrix_local.inverted().to_3x3() @ target_world.inverted().to_3x3()
    action = bpy.data.actions.new(path.stem)
    action.use_fake_user = True
    arm.animation_data.action = action
    previous = {}
    start, end = map(round, source_action.frame_range)
    for frame in range(start, end + 1):
        bpy.context.scene.frame_set(frame)
        desired = {}
        for name in names:
            sp = (source.matrix_world @ source.pose.bones[name].matrix).to_quaternion()
            world_q = align @ sp @ source_rest[name].inverted() @ align.inverted() @ aligned_rest[name]
            desired[name] = world_rotation_inverse @ world_q
        for name in names:
            pb = arm.pose.bones[name]
            pre = (desired[pb.parent.name] @ rest_local[name]) if pb.parent else rest_local[name]
            q = pre.inverted() @ desired[name]
            q.normalize()
            if name in previous and q.dot(previous[name]) < 0:
                q.negate()
            previous[name] = q.copy()
            pb.rotation_quaternion = q
            pb.keyframe_insert(data_path='rotation_quaternion', frame=frame, group=name)
        source_position = source.matrix_world @ source.pose.bones[root_name].matrix.translation
        arm.pose.bones[root_name].location = root_delta_basis @ (align @ (source_position - source_hip) * ratio)
        arm.pose.bones[root_name].keyframe_insert(data_path='location', frame=frame, group=root_name)
    for fc in curves(action):
        for point in fc.keyframe_points:
            point.interpolation = 'LINEAR'
    for obj in imported:
        bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.actions.remove(source_action)
    clips.append(action)
    print('RETARGETED', path.stem, end - start + 1, 'frames', len(names), 'bones', flush=True)

arm.animation_data.action = None
for pb in arm.pose.bones:
    pb.matrix_basis.identity()
for action in clips:
    track = arm.animation_data.nla_tracks.new()
    track.name = action.name
    strip = track.strips.new(action.name, int(action.frame_range[0]), action)
    strip.action_slot = action.slots[0]
    track.mute = True
bpy.context.scene.render.fps = 30
output.parent.mkdir(exist_ok=True, parents=True)
bpy.ops.export_scene.gltf(filepath=str(output), export_format='GLB', export_animation_mode='NLA_TRACKS',
                          export_force_sampling=True, export_apply=False)
print('EXPORTED', output, output.stat().st_size, flush=True)
