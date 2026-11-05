"""Detect contact bridges/weight tears by sampling actual evaluated mesh edges."""
import bpy,sys,json
import numpy as np
from pathlib import Path
arguments=sys.argv[sys.argv.index('--')+1:];path=Path(arguments[0])
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.context.scene.render.fps=30;bpy.ops.import_scene.gltf(filepath=str(path))
arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for track in arm.animation_data.nla_tracks:track.mute=True
meshes=[o for o in bpy.context.scene.objects if o.type=='MESH' and o.vertex_groups]
profile_path=Path(arguments[1]) if len(arguments)>1 else path.with_suffix('.collision.json')
report_path=Path(arguments[2]) if len(arguments)>2 else path.with_suffix('.deformation.json')
profile=json.loads(profile_path.read_text())['profile'];height=profile['height']
references={}
for mesh in meshes:
    vertices=np.empty(len(mesh.data.vertices)*3,dtype=np.float32);mesh.data.vertices.foreach_get('co',vertices);vertices=vertices.reshape(-1,3)
    edges=np.empty(len(mesh.data.edges)*2,dtype=np.int32);mesh.data.edges.foreach_get('vertices',edges);edges=edges.reshape(-1,2)
    lengths=np.linalg.norm(vertices[edges[:,0]]-vertices[edges[:,1]],axis=1)
    references[mesh.name]=edges[(lengths>height*0.00001)&(lengths<height*0.025)]
rows=[]
for action in list(bpy.data.actions):
    arm.animation_data.action=action;arm.animation_data.action_slot=action.slots[0]
    bad=0;maximum=0
    start,end=action.frame_range
    for fraction in (0.15,0.35,0.6,0.85):
        bpy.context.scene.frame_set(round(start+(end-start)*fraction))
        graph=bpy.context.evaluated_depsgraph_get()
        for mesh in meshes:
            evaluated=mesh.evaluated_get(graph);data=evaluated.to_mesh()
            vertices=np.empty(len(data.vertices)*3,dtype=np.float32);data.vertices.foreach_get('co',vertices);vertices=vertices.reshape(-1,3)
            edges=references[mesh.name];lengths=np.linalg.norm(vertices[edges[:,0]]-vertices[edges[:,1]],axis=1)
            maximum=max(maximum,float(lengths.max(initial=0)));bad+=int(np.count_nonzero(lengths>height*0.1))
            evaluated.to_mesh_clear()
    row={'clip':action.name,'rupturedEdgeSamples':bad,'maxShortEdgeLength':maximum};rows.append(row)
    print('DEFORMATION',json.dumps(row),flush=True)
report_path.write_text(json.dumps(rows,indent=2)+'\n')
assert sum(r['rupturedEdgeSamples'] for r in rows)==0,'Ruptured/contact-bridge geometry needs individual repair'
print('PASS evaluated-mesh deformation checks',len(rows),'clips',flush=True)
