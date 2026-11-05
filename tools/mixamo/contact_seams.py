"""Separate fused hand/hip contacts without moving the collected model's surface."""
import bmesh
from collections import defaultdict
from mathutils import Vector


def smooth_shoulders(arm,meshes,height):
    import heapq,math
    changed=0
    for mesh in meshes:
        vertices=list(mesh.data.vertices);count=len(vertices)
        matrix=arm.matrix_world.inverted()@mesh.matrix_world
        positions=[matrix@v.co for v in vertices]
        names={g.index:g.name for g in mesh.vertex_groups};ids={n:i for i,n in names.items()}
        weights=[{g.group:g.weight for g in v.groups} for v in vertices]
        graph=[{} for _ in vertices]
        for edge in mesh.data.edges:
            a,b=edge.vertices;length=max((positions[a]-positions[b]).length,height*1e-7)
            graph[a][b]=length;graph[b][a]=length
        duplicates=defaultdict(list)
        for i,p in enumerate(positions):duplicates[tuple(round(c,5) for c in p)].append(i)
        for group in duplicates.values():
            if len(group)<2:continue
            for a in group:
                for b in group:
                    if a!=b and (positions[a]-positions[b]).length<height*0.00003:
                        # Preserve the deliberate hand/hip cut below the elbows.
                        if max(positions[a].z,positions[b].z)>min(arm.data.bones['mixamorig:'+q+'Arm'].head_local.z for q in ('Left','Right'))-height*0.14:
                            graph[a][b]=height*1e-7
        def distance(role,bone,child):
            anchor=(arm.data.bones[bone].head_local+arm.data.bones[child].head_local)*0.5
            group_ids=[ids[n] for n in role if n in ids]
            if bone=='mixamorig:Spine1':
                candidates=[i for i,w in enumerate(weights) if sum(value for g,value in w.items() if not any(names[g].startswith('mixamorig:'+side+n) for side in ('Left','Right') for n in ('Shoulder','Arm','ForeArm','Hand')))>0.65]
                seeds=candidates
            else:
                candidates=[i for i,w in enumerate(weights) if sum(w.get(g,0) for g in group_ids)>0.65]
                seeds=sorted(candidates,key=lambda i:(positions[i]-anchor).length_squared)[:12]
            d=[float('inf')]*count;queue=[]
            for i in seeds:d[i]=0;heapq.heappush(queue,(0,i))
            while queue:
                value,i=heapq.heappop(queue)
                if value!=d[i]:continue
                for j,cost in graph[i].items():
                    new=value+cost
                    if new<d[j]:d[j]=new;heapq.heappush(queue,(new,j))
            return d
        body=distance(['mixamorig:Spine','mixamorig:Spine1','mixamorig:Spine2'],'mixamorig:Spine1','mixamorig:Spine2')
        solutions={}
        for side in ('Left','Right'):
            prefix='mixamorig:'+side;shoulder=arm.data.bones[prefix+'Arm'].head_local
            roles=[('mixamorig:Spine2',body),
                   (prefix+'Shoulder',distance([prefix+'Shoulder'],prefix+'Shoulder',prefix+'Arm')),
                   (prefix+'Arm',distance([prefix+'Arm'],prefix+'Arm',prefix+'ForeArm')),
                   (prefix+'ForeArm',distance([prefix+'ForeArm'],prefix+'ForeArm',prefix+'Hand'))]
            for i,p in enumerate(positions):
                r=(p-shoulder).length/height
                if not shoulder.z-height*0.17<p.z<shoulder.z+height*0.04 or r>0.2:continue
                hand=sum(w for g,w in weights[i].items() if 'Hand' in names[g])
                head=sum(w for g,w in weights[i].items() if names[g].endswith(('Head','Neck')))
                def smoothstep(a,b,x):
                    t=max(0,min(1,(x-a)/(b-a)));return t*t*(3-2*t)
                protection=(1-smoothstep(0.1,0.7,hand))*(1-smoothstep(0.01,0.2,head))
                influence=max(0,min(1,(0.2-r)/0.05))
                influence*=max(0,min(1,(p.z-(shoulder.z-height*0.17))/(height*0.09)))
                influence*=protection
                raw={ids[n]:1/(d[i]+height*0.018)**4 for n,d in roles if math.isfinite(d[i])}
                amount=sum(raw.values())
                if not amount:continue
                target={g:w/amount for g,w in raw.items()}
                result=defaultdict(float)
                for g,w in weights[i].items():result[g]+=w*(1-influence)
                for g,w in target.items():result[g]+=w*influence
                pairs=sorted(result.items(),key=lambda x:-x[1])[:4];amount=sum(w for _,w in pairs)
                candidate={g:w/amount for g,w in pairs}
                if i not in solutions or r<solutions[i][0]:solutions[i]=(r,candidate)
        for i,(_,result) in solutions.items():
            for g in [g.group for g in mesh.data.vertices[i].groups]:mesh.vertex_groups[g].remove([i])
            for g,w in result.items():mesh.vertex_groups[g].add([i],w,'REPLACE')
        changed+=len(solutions)
    return changed


def separate(arm,meshes,height):
    total_cut=0;total_caps=0
    for mesh in meshes:
        names={g.index:g.name for g in mesh.vertex_groups};indices={v:k for k,v in names.items()}
        bm=bmesh.new();bm.from_mesh(mesh.data);layer=bm.verts.layers.deform.verify()
        uv=bm.loops.layers.uv.active
        transform=arm.matrix_world.inverted()@mesh.matrix_world
        hands={s:arm.data.bones['mixamorig:'+s+'Hand'].head_local.copy() for s in ('Left','Right')}
        shoulders={s:arm.data.bones['mixamorig:'+s+'Arm'].head_local.z for s in ('Left','Right')}
        def region(v,side):
            p=transform@v.co
            return p.z<shoulders[side]-height*0.09 and (p-hands[side]).length<height*0.19
        contact=[v for v in bm.verts if any(region(v,s) for s in hands)]
        bmesh.ops.remove_doubles(bm,verts=contact,dist=height*0.000002)
        classification={}
        for v in bm.verts:
            weights=v[layer]
            scores={s:sum(w for i,w in weights.items() if any(names.get(i,'').startswith('mixamorig:'+s+n) for n in ('Shoulder','Arm','ForeArm','Hand'))) for s in hands}
            side=max(scores,key=scores.get)
            classification[v]=side if scores[side]>0.5 else None
        mixed=set()
        for f in bm.faces:
            labels={classification[v] for v in f.verts}
            if None not in labels or len(labels)<2:continue
            mixed.add(f)
        faces=[];todo=set(mixed)
        while todo:
            seed=todo.pop();component={seed};pending=[seed]
            while pending:
                current=pending.pop()
                for edge in current.edges:
                    for neighbor in edge.link_faces:
                        if neighbor in todo:todo.remove(neighbor);component.add(neighbor);pending.append(neighbor)
            contact=any(any(region(v,s) for v in f.verts for s in hands if classification[v]==s) for f in component)
            if contact:faces.extend(component)
        if not faces:bm.free();continue
        candidates={edge for f in faces for edge in f.edges}
        uvs={v:next((loop[uv].uv.copy() for loop in v.link_loops),Vector((0,0))) for v in bm.verts} if uv else {}
        bmesh.ops.delete(bm,geom=faces,context='FACES_ONLY')
        boundary=[e for e in candidates if e.is_valid and len(e.link_faces)==1]
        caps=[]
        for label in (None,'Left','Right'):
            ring=[e for e in boundary if all(classification[v]==label for v in e.verts)]
            if ring:caps.extend(bmesh.ops.holes_fill(bm,edges=ring,sides=0)['faces'])
        for f in caps:
            f.smooth=True
            if uv:
                for loop in f.loops:loop[uv].uv=uvs.get(loop.vert,Vector((0,0)))
        for v in bm.verts:
            side=classification.get(v)
            if not any(region(v,s) for s in hands):continue
            p=transform@v.co
            if not any(p.z<hands[s].z+height*0.035 and (p-hands[s]).length<height*0.13 for s in hands):continue
            weights=v[layer]
            if side:
                keep={i:w for i,w in weights.items() if any(names.get(i,'').startswith('mixamorig:'+side+n) for n in ('Shoulder','Arm','ForeArm','Hand'))}
            else:
                keep={i:w for i,w in weights.items() if not any(names.get(i,'').startswith('mixamorig:'+s+'Hand') for s in hands)}
            amount=sum(keep.values())
            if amount:
                weights.clear()
                for i,w in keep.items():weights[i]=w/amount
        bmesh.ops.recalc_face_normals(bm,faces=list(bm.faces))
        bm.to_mesh(mesh.data);mesh.data.update();bm.free()
        total_cut+=len(faces);total_caps+=len(caps)
    return {'removedBridgeFaces':total_cut,'cappedContactFaces':total_caps}
