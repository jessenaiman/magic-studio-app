"""Independently evaluate exported arm contacts, finite poses and bone lengths."""
import bpy, sys, json, math
from pathlib import Path
from mathutils import Vector, Matrix
from mathutils.bvhtree import BVHTree
sys.path.insert(0,str(Path(__file__).resolve().parent))
from collision_fit import PREFIX
from arm_collisions import score

arguments=sys.argv[sys.argv.index('--')+1:]
path=Path(arguments[0])
profile_path=Path(arguments[1]) if len(arguments)>1 else path.with_suffix('.collision.json')
report_path=Path(arguments[2]) if len(arguments)>2 else path.with_suffix('.review.json')
data=json.loads(profile_path.read_text());profile=data['profile']
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for action in list(bpy.data.actions):bpy.data.actions.remove(action)
bpy.context.scene.render.fps=30;bpy.ops.import_scene.gltf(filepath=str(path))
arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for track in arm.animation_data.nla_tracks:track.mute=True
rest={b.name:b.matrix_local.copy() for b in arm.data.bones}
trees={p['name']:BVHTree.FromPolygons(p['surface']['vertices'],p['surface']['triangles'],all_triangles=True) for p in profile['proxies']}
results=[]
for action in list(bpy.data.actions):
    arm.animation_data.action=action;arm.animation_data.action_slot=action.slots[0]
    contacts=0;worst=0;length_error=0;step=0;previous={};step_bone='';step_frame=0
    for frame in range(round(action.frame_range[0]),round(action.frame_range[1])+1):
        bpy.context.scene.frame_set(frame)
        poses={p.name:p.matrix.copy() for p in arm.pose.bones}
        for name,matrix in poses.items():
            assert all(math.isfinite(v) for row in matrix for v in row),(action.name,frame,name)
        colliders=[]
        for proxy in profile['proxies']:
            bone=proxy['bone'];delta=poses[bone].to_quaternion()@rest[bone].to_quaternion().inverted()
            if profile.get('collisionMode')=='ellipsoid-pbd':
                center=poses[bone].translation+delta@(Vector(proxy['center'])-rest[bone].translation)
                colliders.append((center,delta.to_matrix()@Matrix(proxy['axes']),Vector(proxy['radii'])))
            else:
                colliders.append({'tree':trees[proxy['name']],'delta':delta,'origin':rest[bone].translation,'posed':poses[bone].translation})
        for side in ('Left','Right'):
            upper,lower,hand=(PREFIX+side+suffix for suffix in ('Arm','ForeArm','Hand'))
            s,e,w=(poses[n].translation for n in (upper,lower,hand))
            finger=PREFIX+side+'HandMiddle2'
            palm=(poses[finger].translation-w)*0.75 if finger in poses else poses[hand].to_quaternion()@Vector((0,profile['height']*0.035,0))
            penalty=math.sqrt(score(s,e,w,palm,profile['armRadii'][side],colliders))
            if penalty>0.001:contacts+=1
            worst=max(worst,penalty)
            for a,b in [(upper,lower),(lower,hand)]:
                length_error=max(length_error,abs((poses[b].translation-poses[a].translation).length-(rest[b].translation-rest[a].translation).length))
            for name in (upper,lower,hand):
                q=poses[name].to_quaternion()
                if name in previous:
                    angle=math.degrees(previous[name].rotation_difference(q).angle)
                    delta=min(angle,360-angle)
                    if delta>step:step=delta;step_bone=name;step_frame=frame
                previous[name]=q
    assert length_error<profile['height']*0.0005,(action.name,'bone stretch',length_error)
    row={'clip':action.name,'contactArmFrames':contacts,'worstPenalty':worst,'maxBoneLengthError':length_error,'maxArmStepDegrees':step,'maxStepBone':step_bone,'maxStepFrame':step_frame}
    results.append(row);print('CHECK',json.dumps(row),flush=True)
report_path.write_text(json.dumps(results,indent=2)+'\n')
print('VERIFIED finite poses and preserved bone lengths;',sum(x['contactArmFrames'] for x in results),'contact arm-frames need review',flush=True)
