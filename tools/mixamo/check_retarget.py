"""Blender: verify exported joint rotations and hip travel against Mixamo sources."""
import bpy, sys, math, json
from pathlib import Path
from mathutils import Vector, Quaternion

target_path, source_path, clip_name = sys.argv[sys.argv.index('--') + 1:]
bpy.ops.object.select_all(action='SELECT'); bpy.ops.object.delete(use_global=False)
for action in list(bpy.data.actions):bpy.data.actions.remove(action)
bpy.context.scene.render.fps = 30
bpy.ops.import_scene.gltf(filepath=target_path)
target = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
clips = {a.name.split('|')[-1]: a for a in bpy.data.actions}
for track in target.animation_data.nla_tracks: track.mute = True

def left(arm):
    v = arm.matrix_world.to_3x3() @ (arm.data.bones['mixamorig:LeftArm'].head_local - arm.data.bones['mixamorig:RightArm'].head_local)
    return math.atan2(v.y, v.x)

paths = sorted(Path(source_path).glob('*.fbx')) if clip_name == '*' else [Path(source_path)]
results = []
for path in paths:
    name = path.stem if clip_name == '*' else clip_name
    clip = clips[name]
    target.animation_data.action = clip
    target.animation_data.action_slot = clip.slots[0]
    before = set(bpy.data.objects)
    bpy.ops.import_scene.fbx(filepath=str(path))
    imported = set(bpy.data.objects) - before
    source = next(o for o in imported if o.type == 'ARMATURE')
    source_clip = source.animation_data.action
    align = Quaternion(Vector((0, 0, 1)), left(target) - left(source))
    aligned_rest={b.name:(target.matrix_world@b.matrix_local).to_quaternion() for b in target.data.bones}
    def child_name(name):
        if name.endswith('Hand'):return name+'Middle1'
        children=[b.name for b in target.data.bones[name].children if b.name in source.data.bones]
        return children[0] if children else None
    deviations=[]
    for side in ('Left','Right'):
        for part in ('Arm','ForeArm'):
            bn='mixamorig:'+side+part;child=child_name(bn)
            td=target.matrix_world.to_3x3()@(target.data.bones[child].head_local-target.data.bones[bn].head_local)
            sd=source.matrix_world.to_3x3()@(source.data.bones[child].head_local-source.data.bones[bn].head_local)
            deviations.append(math.degrees(td.angle(align@sd)))
    if max(deviations)>20:
        for bn in aligned_rest:
            if not any(bn.startswith('mixamorig:'+side+n) for side in ('Left','Right') for n in ('Shoulder','Arm','ForeArm','Hand')):continue
            child=child_name(bn)
            if child:
                td=target.matrix_world.to_3x3()@(target.data.bones[child].head_local-target.data.bones[bn].head_local)
                sd=source.matrix_world.to_3x3()@(source.data.bones[child].head_local-source.data.bones[bn].head_local)
                if td.length>1e-6 and sd.length>1e-6:aligned_rest[bn]=td.rotation_difference(align@sd)@aligned_rest[bn]
    errors, positions = [], []
    trhip = target.matrix_world @ target.data.bones['mixamorig:Hips'].head_local
    srhip = source.matrix_world @ source.data.bones['mixamorig:Hips'].head_local
    ratio = trhip.z / srhip.z
    for fraction in (0.2, 0.5, 0.8):
        frame = round(source_clip.frame_range[0] + fraction * (source_clip.frame_range[1] - source_clip.frame_range[0]))
        bpy.context.scene.frame_set(frame)
        for bone in target.data.bones:
            bn = bone.name
            sr = (source.matrix_world @ source.data.bones[bn].matrix_local).to_quaternion()
            sp = (source.matrix_world @ source.pose.bones[bn].matrix).to_quaternion()
            tr = aligned_rest[bn]
            tp = (target.matrix_world @ target.pose.bones[bn].matrix).to_quaternion()
            want = align @ sp @ sr.inverted() @ align.inverted() @ tr
            error = math.degrees(want.rotation_difference(tp).angle)
            errors.append(abs(min(error, 360 - error)))
        spos = source.matrix_world @ source.pose.bones['mixamorig:Hips'].head
        tpos = target.matrix_world @ target.pose.bones['mixamorig:Hips'].head
        positions.append((tpos - (trhip + align @ (spos - srhip) * ratio)).length)
    print('FIDELITY', name, 'rotation_deg', max(errors), 'hip_error', max(positions), flush=True)
    assert max(errors) < 1, (name, 'world rotations', max(errors))
    assert max(positions) < max(trhip.z * 0.001, 0.0001), (name, 'hip travel', max(positions))
    results.append({'clip':name,'maxRotationErrorDegrees':max(errors),'maxHipError':max(positions)})
    for obj in imported: bpy.data.objects.remove(obj, do_unlink=True)
    bpy.data.actions.remove(source_clip)
print('PASS world-space retarget fidelity:', len(results), 'clips', flush=True)
Path(target_path).with_suffix('.qa.json').write_text(json.dumps(results, indent=2) + '\n')
