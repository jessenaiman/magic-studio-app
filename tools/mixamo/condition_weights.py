"""Condition only skin-weight discontinuities proven to tear in sampled poses."""
import bpy,sys,json,shutil
import numpy as np
from pathlib import Path
from collections import defaultdict
from mathutils import Matrix
source,output=map(Path,sys.argv[sys.argv.index('--')+1:][:2])
bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
for a in list(bpy.data.actions):bpy.data.actions.remove(a)
bpy.context.scene.render.fps=30;bpy.ops.import_scene.gltf(filepath=str(source))
arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
for track in arm.animation_data.nla_tracks:track.mute=True
actions=list(bpy.data.actions);meshes=[o for o in bpy.context.scene.objects if o.type=='MESH' and o.vertex_groups]
height=json.loads(source.with_suffix('.collision.json').read_text())['profile']['height']
references={};weights={}
for mesh in meshes:
    vertices=np.empty(len(mesh.data.vertices)*3,dtype=np.float32);mesh.data.vertices.foreach_get('co',vertices);vertices=vertices.reshape(-1,3)
    edges=np.empty(len(mesh.data.edges)*2,dtype=np.int32);mesh.data.edges.foreach_get('vertices',edges);edges=edges.reshape(-1,2)
    length=np.linalg.norm(vertices[edges[:,0]]-vertices[edges[:,1]],axis=1)
    references[mesh.name]=(edges,length,(length>height*.00001)&(length<height*.025))
    weights[mesh.name]=[{g.group:g.weight for g in v.groups} for v in mesh.data.vertices]
history=[]
for iteration in range(16):
    failures={m.name:np.zeros(len(references[m.name][0]),dtype=bool) for m in meshes}
    maximum=0.0
    for action in actions:
        arm.animation_data.action=action;arm.animation_data.action_slot=action.slots[0]
        start,end=action.frame_range
        for fraction in (.05,.15,.25,.35,.45,.6,.75,.9):
            bpy.context.scene.frame_set(round(start+(end-start)*fraction));graph=bpy.context.evaluated_depsgraph_get()
            for mesh in meshes:
                evaluated=mesh.evaluated_get(graph);data=evaluated.to_mesh()
                v=np.empty(len(data.vertices)*3,dtype=np.float32);data.vertices.foreach_get('co',v);v=v.reshape(-1,3)
                edges,length,small=references[mesh.name]
                posed=np.linalg.norm(v[edges[:,0]]-v[edges[:,1]],axis=1)
                bad=small&(posed>np.maximum(height*.075,length*6))
                failures[mesh.name]|=bad
                maximum=max(maximum,float(posed[small].max(initial=0)))
                evaluated.to_mesh_clear()
    count=sum(int(x.sum()) for x in failures.values())
    history.append({'pass':iteration,'badEdges':count,'maxShortEdgeLength':maximum})
    print('CONDITION',history[-1],flush=True)
    if count==0:break
    for mesh in meshes:
        neighbors=defaultdict(set);edges=references[mesh.name][0]
        for a,b in edges[failures[mesh.name]]:neighbors[int(a)].add(int(b));neighbors[int(b)].add(int(a))
        old=weights[mesh.name];updates={}
        for i,adjacent in neighbors.items():
            average=defaultdict(float)
            for g,w in old[i].items():average[g]+=w*.25
            factor=.75/(len(adjacent)+1)
            for j in [i,*adjacent]:
                for g,w in old[j].items():average[g]+=w*factor
            pairs=sorted(average.items(),key=lambda x:-x[1])[:4];amount=sum(w for _,w in pairs)
            updates[i]={g:w/amount for g,w in pairs}
        for i,new in updates.items():
            for g in [g.group for g in mesh.data.vertices[i].groups]:mesh.vertex_groups[g].remove([i])
            for g,w in new.items():mesh.vertex_groups[g].add([i],w,'REPLACE')
            old[i]=new
        mesh.data.update()
arm.animation_data.action=None
for pb in arm.pose.bones:pb.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update()
output.parent.mkdir(parents=True,exist_ok=True)
bpy.ops.export_scene.gltf(filepath=str(output),export_format='GLB',export_animation_mode='NLA_TRACKS',export_force_sampling=True,export_apply=False)
shutil.copy2(source.with_suffix('.collision.json'),output.with_suffix('.collision.json'))
output.with_suffix('.conditioning.json').write_text(json.dumps(history,indent=2)+'\n')
print('CONDITIONED_EXPORT',output,flush=True)
