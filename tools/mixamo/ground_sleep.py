"""Lift the sleeping clip by one constant offset to support the actual head/skin."""
import bpy,sys,json,shutil
import numpy as np
from pathlib import Path
from mathutils import Vector,Matrix
source,out=map(Path,sys.argv[sys.argv.index('--')+1:][:2]);bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.context.scene.render.fps=30;bpy.ops.import_scene.gltf(filepath=str(source));arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for t in list(arm.animation_data.nla_tracks):arm.animation_data.nla_tracks.remove(t)
arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update();meshes=[o for o in bpy.context.scene.objects if o.type=='MESH' and o.vertex_groups]
def bottom():
 graph=bpy.context.evaluated_depsgraph_get();minimum=float('inf')
 for m in meshes:
  e=m.evaluated_get(graph);data=e.to_mesh();v=np.empty(len(data.vertices)*3,dtype=np.float32);data.vertices.foreach_get('co',v);v=v.reshape(-1,3);matrix=np.array(e.matrix_world);minimum=min(minimum,float((v@matrix[2,:3]+matrix[2,3]).min()));e.to_mesh_clear()
 return minimum
floor=bottom();old=bpy.data.actions['sleeping-idle'];arm.animation_data.action=old;arm.animation_data.action_slot=old.slots[0];minimum=float('inf');samples=[]
for f in range(round(old.frame_range[0]),round(old.frame_range[1])+1):
 bpy.context.scene.frame_set(f);minimum=min(minimum,bottom());samples.append((f,arm.pose.bones['mixamorig:Hips'].location.copy()))
lift=max(0,floor-minimum)+.0005
root=arm.pose.bones['mixamorig:Hips'];assert root.parent is None
basis_offset=root.bone.matrix_local.to_3x3().inverted()@arm.matrix_world.to_3x3().inverted()@Vector((0,0,lift))
old.name='sleeping-idle__source';new=old.copy();new.name='sleeping-idle';arm.animation_data.action=new;arm.animation_data.action_slot=new.slots[0]
for f,loc in samples:
 root.location=loc+basis_offset;root.keyframe_insert(data_path='location',frame=f,group=root.name)
bpy.data.actions.remove(old);arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
for a in list(bpy.data.actions):
 t=arm.animation_data.nla_tracks.new();t.name=a.name;s=t.strips.new(a.name,round(a.frame_range[0]),a);s.action_slot=a.slots[0];t.mute=True
out.parent.mkdir(parents=True,exist_ok=True);bpy.ops.export_scene.gltf(filepath=str(out),export_format='GLB',export_animation_mode='NLA_TRACKS',export_force_sampling=True,export_apply=False)
shutil.copy2(source.with_suffix('.collision.json'),out.with_suffix('.collision.json'))
out.with_suffix('.grounding.json').write_text(json.dumps({'clip':'sleeping-idle','bindFloor':floor,'beforeMinimum':minimum,'constantLift':lift,'framesChecked':len(samples)},indent=2)+'\n');print('GROUNDED_SLEEP',out,'lift',lift,flush=True)
