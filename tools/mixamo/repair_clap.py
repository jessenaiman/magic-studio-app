"""Fit the original clap timing to each character's hand size and chest reach."""
import bpy,sys,json,math,shutil
from pathlib import Path
from mathutils import Vector,Matrix,Quaternion
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[1];sys.path.insert(0,str(ROOT/'tools/mixamo'))
from arm_collisions import two_bone,score
P='mixamorig:'
source,out=map(Path,sys.argv[sys.argv.index('--')+1:][:2])
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.context.scene.render.fps=30
bpy.ops.import_scene.fbx(filepath=str(ROOT/'assets/animations/mixamo/clapping.fbx'))
sa=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');act=sa.animation_data.action;gaps=[]
for f in range(round(act.frame_range[0]),round(act.frame_range[1])+1):
 bpy.context.scene.frame_set(f);gaps.append((sa.pose.bones[P+'LeftHand'].head-sa.pose.bones[P+'RightHand'].head).length)
low,high=min(gaps),max(gaps);phase=[(v-low)/(high-low) for v in gaps]
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.ops.import_scene.gltf(filepath=str(source));arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');actions=list(bpy.data.actions)
for t in list(arm.animation_data.nla_tracks):arm.animation_data.nla_tracks.remove(t)
arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update()
profile=json.loads(source.with_suffix('.collision.json').read_text())['profile'];H=profile['height'];forward=Vector(profile['forward']);left=Vector(profile['left']);up=forward.cross(left).normalized()
rest={b.name:b.matrix_local.copy() for b in arm.data.bones};local={b.name:((b.parent.matrix_local.inverted()@b.matrix_local) if b.parent else b.matrix_local).to_quaternion() for b in arm.data.bones}
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH' and o.vertex_groups];points={}
for side in ('Left','Right'):
 pts=[];hand=P+side+'Hand'
 for m in meshes:
  for v in m.data.vertices:
   if sum(g.weight for g in v.groups if m.vertex_groups[g.group].name.startswith(hand))>.7:
    pts.append(arm.matrix_world.inverted()@m.matrix_world@v.co-rest[hand].translation)
 points[side]=pts
old=next(a for a in actions if a.name=='clapping');old.name='clapping__source';new=old.copy();new.name='clapping';previous={};rows=[]
for frame in range(round(old.frame_range[0]),round(old.frame_range[1])+1):
 arm.animation_data.action=old;arm.animation_data.action_slot=old.slots[0];bpy.context.scene.frame_set(frame)
 rot={b.name:b.matrix.to_quaternion() for b in arm.pose.bones};pos={b.name:b.head.copy() for b in arm.pose.bones}
 delta=rot[P+'Spine1']@rest[P+'Spine1'].to_quaternion().inverted();fw=delta@forward;lf=delta@left;uz=delta@up
 center=(pos[P+'LeftArm']+pos[P+'RightArm'])*.5-uz*(H*.025)
 colliders=[]
 for proxy in profile['proxies']:
  bn=proxy['bone'];q=rot[bn]@rest[bn].to_quaternion().inverted();colliders.append((pos[bn]+q@(Vector(proxy['center'])-rest[bn].translation),q.to_matrix()@Matrix(proxy['axes']),Vector(proxy['radii'])))
 arms={};gap=H*.09*phase[min(len(phase)-1,frame-round(old.frame_range[0]))]
 for side,sign in [('Left',1),('Right',-1)]:
  upper,lower,hand=(P+side+n for n in ('Arm','ForeArm','Hand'));s,e,w=(pos[n] for n in (upper,lower,hand));f=P+side+'HandMiddle2'
  palm=pos[f]-w;q=(palm.normalized()).rotation_difference(uz);handrot=q@rot[hand];hd=handrot@rest[hand].to_quaternion().inverted()
  offsets=[(hd@v).dot(lf) for v in points[side]]
  # Match the actual inner hand surfaces at the original source's clap beats.
  inward=min(offsets) if sign==1 else max(offsets)
  goal=center+lf*(-inward+sign*gap*.5)
  arms[side]=(upper,lower,hand,s,e,w,q,handrot,goal,q@(palm*.75))
 choices=[]
 for protract in range(0,71,10):
  for depth in [H*(.10+i*.005) for i in range(51)]:
   solved={};penalty=0.;cost=0.
   for side,sign in [('Left',1),('Right',-1)]:
    upper,lower,hand,s,e,w,q,hr,goal,palm=arms[side]
    clavicle=P+side+'Shoulder';base=pos[clavicle];cq=Quaternion(uz,math.radians(-sign*protract));ns=base+cq@(s-base);offset=ns-s
    target=goal+fw*depth
    ne,nw=two_bone(ns,target,e+offset+lf*(sign*H*.04),(e-s).length,(w-e).length,lf*sign)
    axis=(nw-ns).normalized();options=[]
    for turn in range(-180,181,15):
     ce=ns+Quaternion(axis,math.radians(turn))@(ne-ns)
     contact=score(ns,ce,nw,palm,profile['armRadii'][side],colliders)
     continuity=(ce-previous[side][0]).length_squared if side in previous else (ce-e).length_squared
     options.append((contact*10000+continuity,contact,ce))
    _,contact,ne=min(options,key=lambda x:x[0])
    penalty+=contact
    cost+=(nw-target).length_squared*100+(ne-e).length_squared*.1+protract*protract*.000001
    if side in previous:cost+=(ne-previous[side][0]).length_squared+(nw-previous[side][1]).length_squared
    solved[side]=(ne,nw,ns,cq)
   choices.append((penalty*10000+cost,penalty,solved,depth,protract))
 _,penalty,solved,depth,protract=min(choices,key=lambda x:x[0])
 for side in ('Left','Right'):
  upper,lower,hand,s,e,w,q,hr,goal,palm=arms[side];ne,nw,ns,cq=solved[side];rot[P+side+'Shoulder']=cq@rot[P+side+'Shoulder'];previous[side]=(ne.copy(),nw.copy())
  rot[upper]=(e-s).rotation_difference(ne-ns)@rot[upper];rot[lower]=(w-e).rotation_difference(nw-ne)@rot[lower]
  for b in arm.data.bones:
   if b.name==hand or any(p.name==hand for p in b.parent_recursive):rot[b.name]=q@rot[b.name]
 arm.animation_data.action=new;arm.animation_data.action_slot=new.slots[0]
 for pb in arm.pose.bones:
  pre=rot[pb.parent.name]@local[pb.name] if pb.parent else local[pb.name];pb.rotation_mode='QUATERNION';pb.rotation_quaternion=pre.inverted()@rot[pb.name];pb.keyframe_insert(data_path='rotation_quaternion',frame=frame,group=pb.name)
 rows.append({'frame':frame,'penalty':penalty,'depth':depth,'protraction':protract})
bpy.data.actions.remove(old)
arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
for a in list(bpy.data.actions):
 t=arm.animation_data.nla_tracks.new();t.name=a.name;s=t.strips.new(a.name,round(a.frame_range[0]),a);s.action_slot=a.slots[0];t.mute=True
out.parent.mkdir(parents=True,exist_ok=True);bpy.ops.export_scene.gltf(filepath=str(out),export_format='GLB',export_animation_mode='NLA_TRACKS',export_force_sampling=True,export_apply=False)
shutil.copy2(source.with_suffix('.collision.json'),out.with_suffix('.collision.json'));out.with_suffix('.clap.json').write_text(json.dumps(rows,indent=2)+'\n');print('CLAP_FIT',out,'max_penalty',max(r['penalty'] for r in rows),flush=True)
