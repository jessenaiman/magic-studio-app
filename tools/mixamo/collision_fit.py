"""Fit local body collision proxies from existing skin weights; never edit geometry."""
import math
from mathutils import Vector, Matrix

PREFIX = 'mixamorig:'


def repair_hand_weights(meshes):
    changed=0
    for mesh in meshes:
        names={g.index:g.name for g in mesh.vertex_groups}
        for vertex in mesh.data.vertices:
            weights={names[g.group]:g.weight for g in vertex.groups if g.group in names}
            totals={s:sum(w for n,w in weights.items() if n.startswith(PREFIX+s+'Hand')) for s in ('Left','Right')}
            side=max(totals,key=totals.get)
            if totals[side]<0.45:continue
            keep={n:w for n,w in weights.items() if n.startswith(PREFIX+side+'Hand') or n==PREFIX+side+'ForeArm'}
            foreign=sum(w for n,w in weights.items() if n not in keep)
            if foreign<0.03:continue
            total=sum(keep.values())
            for name in weights:
                if name not in keep:mesh.vertex_groups[name].remove([vertex.index])
            for name,weight in keep.items():mesh.vertex_groups[name].add([vertex.index],weight/total,'REPLACE')
            changed+=1
    return changed


def repair_torso_weights(arm, meshes, profile):
    """Remove arm influence from clothing in the torso core below the arm sockets."""
    axes=Matrix(profile['proxies'][0]['axes']);inv=axes.transposed()
    torso=profile['proxies'][0];center=inv@Vector(torso['center']);radius=Vector(torso['radii'])
    height=profile['height']
    for side,sign in [('Left',1),('Right',-1)]:
        direction=arm.data.bones[PREFIX+side+'ForeArm'].head_local-arm.data.bones[PREFIX+side+'Arm'].head_local
        if direction.angle(Vector(profile['left'])*sign)>math.radians(20):
            # Hands-on-hips/A-pose rigs put genuine limb vertices beside the
            # waist. A torso-region heuristic cannot separate those safely.
            return 0
    shoulders=[inv@arm.data.bones[PREFIX+s+'Arm'].head_local for s in ('Left','Right')]
    top=min(p.z for p in shoulders)-height*0.025
    bottom=(inv@arm.data.bones[PREFIX+'Hips'].head_local).z-height*0.22
    torso_names=[PREFIX+n for n in ('Hips','Spine','Spine1','Spine2')]
    arm_names={b.name for b in arm.data.bones if any(b.name.startswith(PREFIX+s+n) for s in ('Left','Right') for n in ('Shoulder','Arm','ForeArm','Hand'))}
    changed=0
    for mesh in meshes:
        transform=arm.matrix_world.inverted()@mesh.matrix_world
        names={g.index:g.name for g in mesh.vertex_groups}
        for vertex in mesh.data.vertices:
            p=inv@(transform@vertex.co)
            if not bottom<p.z<top:continue
            if abs(p.y-center.y)>radius.y*1.12:continue
            if abs(p.x-center.x)>radius.x+height*0.12:continue
            weights={names[g.group]:g.weight for g in vertex.groups if g.group in names}
            if sum(w for n,w in weights.items() if 'Hand' in n)>0.4:continue
            transfer=sum(w for n,w in weights.items() if n in arm_names)
            if transfer<0.05:continue
            current={n:weights.get(n,0) for n in torso_names}
            total=sum(current.values())
            if total<0.01:
                nearest=min(torso_names,key=lambda n:abs((inv@arm.data.bones[n].head_local).z-p.z))
                current={n:float(n==nearest) for n in torso_names};total=1
            for name in arm_names:
                if name in weights:mesh.vertex_groups[name].remove([vertex.index])
            for name,weight in current.items():
                if weight:
                    group=mesh.vertex_groups.get(name) or mesh.vertex_groups.new(name=name)
                    group.add([vertex.index],weights.get(name,0)+transfer*weight/total,'REPLACE')
            changed+=1
    return changed


def fit(arm, meshes, head_bands=1, head_min_from_bone=None, torso_core=False):
    left = arm.data.bones[PREFIX+'LeftArm'].head_local - arm.data.bones[PREFIX+'RightArm'].head_local
    up = (arm.matrix_world.inverted().to_3x3() @ Vector((0, 0, 1))).normalized()
    left -= up * left.dot(up)
    left.normalize()
    forward = left.cross(up).normalized()
    axes = Matrix((forward, left, up)).transposed()
    inverse = axes.transposed()
    body, head, arms = [], [], {'Left': [], 'Right': []}
    surfaces={'torso':{'vertices':[],'triangles':[]},'head':{'vertices':[],'triangles':[]}}
    allpoints = []
    torso_names = {PREFIX+n for n in ('Hips', 'Spine', 'Spine1', 'Spine2')}
    for mesh in meshes:
        matrix = arm.matrix_world.inverted() @ mesh.matrix_world
        groups = {g.index: g.name for g in mesh.vertex_groups}
        region_weights={'torso':[],'head':[]}
        mesh_points=[]
        for v in mesh.data.vertices:
            p = matrix @ v.co
            allpoints.append(p)
            weights = {groups[g.group]: g.weight for g in v.groups if g.group in groups}
            region_weights['torso'].append(sum(w for n,w in weights.items() if n in torso_names))
            region_weights['head'].append(weights.get(PREFIX+'Head',0))
            mesh_points.append(p)
            if sum(w for n,w in weights.items() if n in torso_names) > 0.7:
                body.append(inverse @ p)
            if weights.get(PREFIX+'Head',0) > 0.7:
                head.append(inverse @ p)
            for side in arms:
                if weights.get(PREFIX+side+'Arm',0) + weights.get(PREFIX+side+'ForeArm',0) > 0.7:
                    arms[side].append(p)
        mesh.data.calc_loop_triangles()
        for region,surface in surfaces.items():
            mapping={}
            for triangle in mesh.data.loop_triangles:
                if sum(region_weights[region][i] for i in triangle.vertices)/3 < 0.65:continue
                indices=[]
                for i in triangle.vertices:
                    if i not in mapping:
                        mapping[i]=len(surface['vertices']);surface['vertices'].append(list(mesh_points[i]))
                    indices.append(mapping[i])
                surface['triangles'].append(indices)
    height = max(p.dot(up) for p in allpoints) - min(p.dot(up) for p in allpoints)
    if head_min_from_bone is not None:
        minimum=(inverse@arm.data.bones[PREFIX+'Head'].head_local).z+height*head_min_from_bone
        core=[p for p in head if p.z>minimum]
        if len(core)>100:head=core
    if body:
        extent=Vector([max(p[i] for p in body)-min(p[i] for p in body) for i in range(3)])
        if torso_core or max(extent.x,extent.y)>height*0.55 or extent.z>height*0.45:
            # Some rigs weight trousers/tails to Hips. Those are not torso
            # collision surfaces and must not inflate the chest volume.
            hip=inverse@arm.data.bones[PREFIX+'Hips'].head_local
            shoulders=[inverse@arm.data.bones[PREFIX+s+'Arm'].head_local for s in ('Left','Right')]
            half_width=abs(shoulders[0].y-shoulders[1].y)*0.6
            top=min(p.z for p in shoulders)+height*0.02
            core=[p for p in body if hip.z-height*0.08<p.z<top and abs(p.x-hip.x)<height*0.25 and abs(p.y-hip.y)<half_width]
            if len(core)>100:body=core
    proxies = []
    regions=[('torso',body,'Spine1')]
    if head_bands>1:
        low_z=min(p.z for p in head);high_z=max(p.z for p in head);span=high_z-low_z
        fractions=(0.2,0.5,0.8) if head_bands==3 else tuple((i+0.5)/head_bands for i in range(head_bands))
        window=0.3 if head_bands==3 else 0.8/head_bands
        for i,fraction in enumerate(fractions):
            subset=[p for p in head if abs(p.z-(low_z+span*fraction))<span*window]
            regions.append((f'head-{i}',subset,'Head'))
    else:regions.append(('head',head,'Head'))
    for name, points, bone_name in regions:
        assert points, name
        low=Vector([min(p[i] for p in points) for i in range(3)])
        high=Vector([max(p[i] for p in points) for i in range(3)])
        center=(low+high)/2
        radii=(high-low)/2
        # Fit the bulk of the weighted skin, allowing small garment protrusions.
        lengths=sorted(math.sqrt(sum(((p[i]-center[i])/max(radii[i],1e-6))**2 for i in range(3))) for p in points)
        scale=lengths[int(len(lengths)*0.95)]
        if name.startswith('head-'):
            radii.x*=1.12;radii.y*=1.12;radii.z+=height*0.012
        else:radii *= scale
        # Shoulder roots are deliberately exempt: a limb must join the torso.
        proxies.append({'name':name,'bone':PREFIX+bone_name,'center':list(axes@center),
                        'radii':list(radii),'axes':[list(row) for row in axes], 'surface':surfaces['head' if name.startswith('head') else 'torso']})
    radii = {}
    for side, points in arms.items():
        distances=[]
        for p in points:
            segments=[]
            for a,b in [('Arm','ForeArm'),('ForeArm','Hand')]:
                start=arm.data.bones[PREFIX+side+a].head_local
                end=arm.data.bones[PREFIX+side+b].head_local
                d=end-start
                closest=start+d*max(0,min(1,(p-start).dot(d)/d.length_squared))
                segments.append((p-closest).length)
            distances.append(min(segments))
        distances.sort()
        # Mixed shoulder/garment weights are not a reliable limb diameter.
        # Use the limb's median thickness, bounded by figure height.
        radii[side]=min(distances[int(len(distances)*0.55)],height*0.045) if distances else height*0.035
    return {'height':height,'forward':list(forward),'left':list(left),'proxies':proxies,'armRadii':radii,
            'armWorld':[list(row) for row in arm.matrix_world],'headBands':head_bands,'headMinFromBone':head_min_from_bone}
