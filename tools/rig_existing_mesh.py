import bpy,math,json,sys
from pathlib import Path
from mathutils import Vector,Matrix
import argparse
parser=argparse.ArgumentParser(description='Fit an existing mesh to a reusable biped skeleton and bind through a voxel proxy. Runs inside Blender.')
parser.add_argument('--source',required=True);parser.add_argument('--out',required=True);parser.add_argument('--template-rig',required=True);parser.add_argument('--profile',required=True)
args=parser.parse_args(sys.argv[sys.argv.index('--')+1:])
source_path=Path(args.source).resolve();output=Path(args.out).resolve();profile=json.loads(Path(args.profile).read_text());name=source_path.stem;out=output.parent;out.mkdir(parents=True,exist_ok=True)
if output==source_path:raise ValueError('Use a separate output to preserve the source mesh')
bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath=str(Path(args.template_rig).resolve()))
donor=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE');armdata=donor.data.copy()
for obj in list(bpy.data.objects):bpy.data.objects.remove(obj,do_unlink=True)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.ops.import_scene.gltf(filepath=str(source_path))
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH']
# This asset's front is +X. Keep positions/textures intact in model space, rotate into rig space.
for o in list(bpy.context.scene.objects):
 if o.parent is None:o.matrix_world=Matrix.Rotation(math.radians(profile['yaw_degrees']),4,'Z')@o.matrix_world
bpy.context.view_layer.update()
for o in meshes:
 mw=o.matrix_world.copy();o.parent=None;o.matrix_world=mw;bpy.context.view_layer.objects.active=o;o.select_set(True);bpy.ops.object.transform_apply(location=True,rotation=True,scale=True);o.select_set(False)
pts=[v.co for o in meshes for v in o.data.vertices];lo=Vector(tuple(min(v[i] for v in pts) for i in range(3)));hi=Vector(tuple(max(v[i] for v in pts) for i in range(3)));H=hi.z-lo.z
center=Vector(((hi.x+lo.x)/2,0,lo.z))
for o in meshes:
 for v in o.data.vertices:v.co=(v.co-center)/H
arm=bpy.data.objects.new('AnimalBiped',armdata);bpy.context.collection.objects.link(arm);bpy.context.view_layer.objects.active=arm;arm.select_set(True);bpy.ops.object.mode_set(mode='EDIT')
E=arm.data.edit_bones
find=lambda n:next(b for b in E if b.name.endswith(':'+n) or b.name==n)
coords=profile['bones']
for b in E:
 if not any(b.name.endswith(n) for n in coords):
  side='Left' if 'Left' in b.name else 'Right';sign=1 if side=='Left' else -1
  b.use_connect=False;b.head=(sign*.405,-.005,.62);b.tail=(sign*.43,-.005,.62);b.use_deform=False
for n,(h,t) in coords.items():
 b=find(n);b.use_connect=False;b.head=h;b.tail=t;b.roll=0;b.use_deform=True
bpy.ops.object.mode_set(mode='OBJECT')
# Bind on a watertight voxel proxy; transfer weights back without replacing source geometry.
original=meshes[0];proxy=original.copy();proxy.data=original.data.copy();bpy.context.collection.objects.link(proxy);proxy.name='WeightBindingProxy'
bpy.ops.object.select_all(action='DESELECT');proxy.select_set(True);bpy.context.view_layer.objects.active=proxy
remesh=proxy.modifiers.new('Watertight binding proxy','REMESH');remesh.mode='VOXEL';remesh.voxel_size=profile.get('voxel_size',.008)
bpy.ops.object.modifier_apply(modifier=remesh.name)
arm.select_set(True);bpy.context.view_layer.objects.active=arm;bpy.ops.object.parent_set(type='ARMATURE_AUTO')
if not any(v.groups for v in proxy.data.vertices):raise RuntimeError('Proxy heat binding failed')
for o in meshes:
 for group in proxy.vertex_groups:o.vertex_groups.new(name=group.name)
 bpy.context.view_layer.objects.active=o
 transfer=o.modifiers.new('Transfer proxy skin weights','DATA_TRANSFER');transfer.object=proxy;transfer.use_vert_data=True;transfer.data_types_verts={'VGROUP_WEIGHTS'};transfer.vert_mapping='POLYINTERP_NEAREST'
 bpy.ops.object.modifier_apply(modifier=transfer.name)
 o.parent=arm;mod=o.modifiers.new('Skeleton','ARMATURE');mod.object=arm
 bpy.ops.object.select_all(action='DESELECT');o.select_set(True);bpy.context.view_layer.objects.active=o
 bpy.ops.object.vertex_group_limit_total(limit=4);bpy.ops.object.vertex_group_normalize_all(lock_active=False)
# Tiny detached surfaces can miss the proxy. Copy the nearest weighted original vertex.
from mathutils.kdtree import KDTree
for o in meshes:
 valid=[v for v in o.data.vertices if any(g.weight>0 for g in v.groups)]
 missing=[v for v in o.data.vertices if not any(g.weight>0 for g in v.groups)]
 if len(missing)>len(o.data.vertices)*.002:raise RuntimeError('Too many unmatched skin vertices')
 tree=KDTree(len(valid))
 for v in valid:tree.insert(v.co,v.index)
 tree.balance()
 for v in missing:
  co,index,dist=tree.find(v.co)
  if dist>.03:raise RuntimeError('Detached surface too far from weighted mesh')
  for g in o.data.vertices[index].groups:o.vertex_groups[g.group].add([v.index],g.weight,'REPLACE')
 print('NEAREST_SURFACE_REPAIR',len(missing),flush=True)
bpy.data.objects.remove(proxy,do_unlink=True)
unweighted=[]
for o in meshes:
 n=sum(1 for v in o.data.vertices if not any(g.weight>0 for g in v.groups));unweighted.append([o.name,n])
 if n:raise RuntimeError('Proxy binding left unweighted vertices: '+str(unweighted))
 for suffix in ['LeftArm','RightArm','LeftHand','RightHand','LeftLeg','RightLeg']:
  group=next(g for g in o.vertex_groups if g.name.endswith(suffix));count=sum(1 for v in o.data.vertices if any(g.group==group.index and g.weight>.25 for g in v.groups));print('INFLUENCE',suffix,count,flush=True)
  if count<20:raise RuntimeError('Missing meaningful limb weights: '+suffix)
bpy.ops.wm.save_as_mainfile(filepath=str(output.with_suffix('.blend')))
bpy.ops.export_scene.gltf(filepath=str(output),export_format='GLB',export_animations=False)
output.with_suffix('.rig-report.json').write_text(json.dumps({'source':str(source_path),'method':'Fitted Mixamo-compatible biped skeleton, Blender heat weights transferred from a watertight voxel proxy; mitten fingers non-deforming','bone_count':len(arm.data.bones),'unweighted_vertices':unweighted,'status':'needs-visual-review','archetype':profile['id'],'profile':str(Path(args.profile).resolve())},indent=2))
print('RIG_CANDIDATE',name,'bones',len(arm.data.bones),'unweighted',unweighted,flush=True)
