"""Bake arm collision correction and repair torso skin weights; keep shape and textures."""
import bpy, sys, json, math
from pathlib import Path
from mathutils import Vector, Matrix, Quaternion
from mathutils.bvhtree import BVHTree
sys.path.insert(0,str(Path(__file__).resolve().parent))
from collision_fit import fit, repair_torso_weights, repair_hand_weights, PREFIX
from arm_collisions import score
from pbd_arms import solve

source, output = map(Path,sys.argv[sys.argv.index('--')+1:][:2])
only = set(sys.argv[-1].split(',')) if len(sys.argv[sys.argv.index('--')+1:])>2 else None
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for action in list(bpy.data.actions):bpy.data.actions.remove(action)
bpy.context.scene.render.fps=30
bpy.ops.import_scene.gltf(filepath=str(source))
arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
clips=list(bpy.data.actions)
for track in list(arm.animation_data.nla_tracks):arm.animation_data.nla_tracks.remove(track)
arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update()
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH' and any(m.type=='ARMATURE' and m.object==arm for m in o.modifiers)]
adjustments={};head_bands=1;head_min_from_bone=None;separate_contacts=False;smooth_shoulders_enabled=False;clean_hands=False;clean_torso=False;rig_costume=False;torso_core=False
adjustment_file=Path(__file__).resolve().parent/'character_adjustments.json'
if adjustment_file.exists():
    for key,value in json.loads(adjustment_file.read_text()).items():
        if key in source.stem or any(value.get('meshMatch','__none__') in m.data.name for m in meshes):
            adjustments=value.get('clips',{});head_bands=value.get('headBands',1);head_min_from_bone=value.get('headMinFromBone');separate_contacts=value.get('separateHandHipContacts',False);smooth_shoulders_enabled=value.get('smoothShoulderWeights',False)
            clean_hands=value.get('cleanHandWeights',False);clean_torso=value.get('cleanTorsoWeights',False);rig_costume=value.get('rigCostumeToBody',False);torso_core=value.get('torsoCore',False)
seam_repairs={}
if separate_contacts and not only:
    from contact_seams import separate
    initial_profile=fit(arm,meshes)
    seam_repairs=separate(arm,meshes,initial_profile['height'])
    print('CONTACT_SEAMS',seam_repairs,flush=True)
if smooth_shoulders_enabled:
    from contact_seams import smooth_shoulders
    shoulder_repairs=smooth_shoulders(arm,meshes,fit(arm,meshes)['height'])
    print('SHOULDER_WEIGHTS',shoulder_repairs,flush=True)
costume_repairs=0
if rig_costume:
    from costume_weights import repair
    costume_repairs=repair(arm,meshes,fit(arm,meshes)['height'])
    print('COSTUME_WEIGHTS',costume_repairs,flush=True)
hand_repairs=repair_hand_weights(meshes) if clean_hands and not only else 0
profile=fit(arm,meshes,head_bands=head_bands,head_min_from_bone=head_min_from_bone,torso_core=torso_core)
changed_weights=repair_torso_weights(arm,meshes,profile) if clean_torso else 0
profile=fit(arm,meshes,head_bands=head_bands,head_min_from_bone=head_min_from_bone,torso_core=torso_core)
profile['collisionMode']='ellipsoid-pbd'
print('TORSO_WEIGHT_REPAIRS',changed_weights,flush=True)
trees={p['name']:BVHTree.FromPolygons(p['surface']['vertices'],p['surface']['triangles'],all_triangles=True) for p in profile['proxies']}
rest={b.name:b.matrix_local.copy() for b in arm.data.bones}
local={b.name:((b.parent.matrix_local.inverted()@b.matrix_local) if b.parent else b.matrix_local).to_quaternion() for b in arm.data.bones}
forward=Vector(profile['forward']);left=Vector(profile['left'])
report=[]; finished=[]

for old in clips:
    name=old.name
    if only and name not in only:
        finished.append(old);continue
    old.name=name+'__source'
    new=old.copy();new.name=name;new.use_fake_user=True
    fixed=0; unresolved=0; initial_max=0; final_max=0; angle_max=0
    previous={'Left':None,'Right':None}
    previous_rotation={}
    for frame in range(round(old.frame_range[0]),round(old.frame_range[1])+1):
        arm.animation_data.action=old;arm.animation_data.action_slot=old.slots[0]
        bpy.context.scene.frame_set(frame)
        rotation={p.name:p.matrix.to_quaternion() for p in arm.pose.bones}
        positions={p.name:p.head.copy() for p in arm.pose.bones}
        body_delta=rotation[PREFIX+'Spine1']@rest[PREFIX+'Spine1'].to_quaternion().inverted()
        colliders=[]
        for proxy in profile['proxies']:
            bone=proxy['bone'];delta=rotation[bone]@rest[bone].to_quaternion().inverted()
            center=positions[bone]+delta@(Vector(proxy['center'])-rest[bone].translation)
            axes=delta.to_matrix()@Matrix(proxy['axes'])
            colliders.append((center,axes,Vector(proxy['radii'])))
        for side,sign in [('Left',1),('Right',-1)]:
            upper=PREFIX+side+'Arm';lower=PREFIX+side+'ForeArm';hand=PREFIX+side+'Hand'
            shoulder,elbow,wrist=(positions[n] for n in (upper,lower,hand))
            finger=PREFIX+side+'HandMiddle2'
            palm=(positions[finger]-wrist)*0.75 if finger in positions else rotation[hand]@Vector((0,profile['height']*0.035,0))
            radius=profile['armRadii'][side]
            up=body_delta@(forward.cross(left).normalized())
            proximity=max(0,min(1,((wrist-positions[PREFIX+'Head']).dot(up)+profile['height']*0.25)/(profile['height']*0.1)))
            bias=body_delta@(left*sign)*(adjustments.get(name,{}).get('handOutward',0)*profile['height']*proximity)
            state=previous[side] or {};state['bias']=bias
            ne,nw,np,q,before,after,angle=solve(shoulder,elbow,wrist,palm,radius,colliders,body_delta@forward,body_delta@(left*sign),state)
            previous[side]={'elbow':ne.copy(),'wrist':nw.copy(),'shoulder':shoulder.copy(),'sourceElbow':elbow.copy(),'sourceWrist':wrist.copy()+bias}
            initial_max=max(initial_max,math.sqrt(before));final_max=max(final_max,math.sqrt(after));angle_max=max(angle_max,angle)
            if before>1e-8:fixed+=1
            if after>1e-6:unresolved+=1
            if before>1e-8 or q.angle>1e-6 or (ne-elbow).length+(nw-wrist).length>1e-7:
                rotation[upper]=(elbow-shoulder).rotation_difference(ne-shoulder)@rotation[upper]
                rotation[lower]=(wrist-elbow).rotation_difference(nw-ne)@rotation[lower]
                for bone_name,child_name,direction in [(upper,lower,ne-shoulder),(lower,hand,nw-ne)]:
                    candidate=rotation[bone_name]
                    if bone_name in previous_rotation and direction.length>1e-8:
                        axis=direction.normalized()
                        local_dir=rest[bone_name].to_quaternion().inverted()@(rest[child_name].translation-rest[bone_name].translation).normalized()
                        transverse=Vector((1,0,0));transverse-=local_dir*transverse.dot(local_dir)
                        if transverse.length<1e-4:transverse=local_dir.orthogonal()
                        transverse.normalize()
                        a=candidate@transverse;b=previous_rotation[bone_name]@transverse
                        a-=axis*a.dot(axis);b-=axis*b.dot(axis)
                        if a.length>1e-6 and b.length>1e-6:
                            a.normalize();b.normalize();angle=math.atan2(axis.dot(a.cross(b)),a.dot(b))
                            limit=math.radians(25)
                            if abs(angle)>limit:candidate=Quaternion(axis,angle-math.copysign(limit,angle))@candidate
                    rotation[bone_name]=candidate;previous_rotation[bone_name]=candidate.copy()
                # Preserve the hand/finger pose, carried by the arm clearance rotation.
                for bone in arm.data.bones:
                    if bone.name==hand or any(p.name==hand for p in bone.parent_recursive):
                        rotation[bone.name]=q@rotation[bone.name]
        arm.animation_data.action=new;arm.animation_data.action_slot=new.slots[0]
        for pb in arm.pose.bones:
            pre=(rotation[pb.parent.name]@local[pb.name]) if pb.parent else local[pb.name]
            pb.rotation_mode='QUATERNION';pb.rotation_quaternion=pre.inverted()@rotation[pb.name]
            pb.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=pb.name)
    finished.append(new)
    bpy.data.actions.remove(old)
    row={'clip':name,'adjustedArmFrames':fixed,'unresolvedArmFrames':unresolved,
         'beforeProxyPenalty':initial_max,'afterProxyPenalty':final_max,'maxClearanceDegrees':angle_max}
    report.append(row);print('COLLISION',json.dumps(row),flush=True)

arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
for action in finished:
    track=arm.animation_data.nla_tracks.new();track.name=action.name
    strip=track.strips.new(action.name,round(action.frame_range[0]),action)
    strip.action_slot=action.slots[0];track.mute=True
output.parent.mkdir(exist_ok=True,parents=True)
bpy.ops.export_scene.gltf(filepath=str(output),export_format='GLB',export_animation_mode='NLA_TRACKS',export_force_sampling=True,export_apply=False)
output.with_suffix('.collision.json').write_text(json.dumps({'profile':profile,'torsoWeightRepairs':changed_weights,'handWeightRepairs':hand_repairs,'costumeWeightRepairs':costume_repairs,'contactSeamRepairs':seam_repairs,'clips':report},indent=2)+'\n')
print('EXPORTED',output,flush=True)
