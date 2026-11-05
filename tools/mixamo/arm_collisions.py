"""Body-proxy collision queries and a length-preserving two-bone arm solver."""
import math
from mathutils import Vector, Quaternion, Matrix


def project(point, colliders, radius):
    result = point.copy()
    worst = 0.0
    for collider in colliders:
        if isinstance(collider,dict):
            delta=collider['delta'];origin=collider['origin'];posed=collider['posed']
            rest_point=origin+delta.inverted()@(result-posed)
            nearest,normal,_,distance=collider['tree'].find_nearest(rest_point)
            if nearest is None:continue
            signed=(rest_point-nearest).dot(normal)
            if signed < radius:
                worst=max(worst,min(5,(radius-signed)/max(radius,1e-6)))
                result=posed+delta@(nearest+normal*(radius*1.015)-origin)
            continue
        center,axes,radii=collider
        local = axes.transposed() @ (result-center)
        expanded = radii + Vector((radius, radius, radius))
        distance = math.sqrt(sum((local[i]/expanded[i])**2 for i in range(3)))
        if distance < 1:
            worst = max(worst, 1-distance)
            if distance < 1e-8:
                local = Vector((0, expanded.y, 0)); distance = 1
            result = center + axes @ (local / distance * 1.02)
    return result, worst


def points(shoulder, elbow, wrist, palm, radius):
    # Upper arms join the torso, so their attachment region must overlap it.
    # Constrain the elbow and free forearm/hand instead of separating the seam.
    return [(elbow,radius*0.7),
            (elbow.lerp(wrist,0.5),radius*0.8), (wrist,radius*0.7),
            (wrist+palm,radius*0.8)]


def score(shoulder, elbow, wrist, palm, radius, colliders):
    return sum(project(p, colliders, r)[1]**2 for p,r in points(shoulder,elbow,wrist,palm,radius))


def two_bone(shoulder, target, guide, l1, l2, fallback):
    delta = target-shoulder
    direction = delta.normalized() if delta.length > 1e-8 else fallback.normalized()
    distance = max(abs(l1-l2)+1e-6, min(delta.length,l1+l2-1e-6))
    wrist = shoulder+direction*distance
    along = (l1*l1-l2*l2+distance*distance)/(2*distance)
    pole = guide-shoulder-direction*(guide-shoulder).dot(direction)
    if pole.length < 1e-6:
        pole = fallback-direction*fallback.dot(direction)
    if pole.length < 1e-6:
        pole = direction.orthogonal()
    elbow = shoulder+direction*along+pole.normalized()*math.sqrt(max(0,l1*l1-along*along))
    return elbow, wrist


def solve(shoulder, elbow, wrist, palm, radius, colliders, forward, outward, previous=None):
    """Try minimum whole-arm clearance, then project contacts and solve elbow/wrist IK."""
    initial = score(shoulder,elbow,wrist,palm,radius,colliders)
    if initial < 1e-8:
        if previous is not None:
            angle=previous.angle
            limited=previous.slerp(Quaternion(),min(1,math.radians(6)/max(angle,1e-6)))
            e=shoulder+limited@(elbow-shoulder);w=shoulder+limited@(wrist-shoulder);p=limited@palm
            if score(shoulder,e,w,p,radius,colliders)<1e-8:
                return e,w,p,limited,initial,0.0,math.degrees(limited.angle)
        return elbow,wrist,palm,Quaternion(),initial,initial,0.0
    best = (initial, elbow, wrist, palm, Quaternion(), 0.0)
    # A bounded search avoids stretching the bones or reversing an elbow.
    axes=(forward.normalized(),outward.normalized(),forward.cross(outward).normalized())
    for degrees in range(3,106,3):
        for axis in axes:
            for sign in (1,-1):
                q=Quaternion(axis, math.radians(degrees*sign))
                e=shoulder+q@(elbow-shoulder)
                w=shoulder+q@(wrist-shoulder)
                p=q@palm
                value=score(shoulder,e,w,p,radius,colliders)
                if value < best[0]: best=(value,e,w,p,q,float(degrees))
        if best[0] < 1e-8: break
    value,e,w,p,q,degrees=best
    if value < 1e-8:
        if previous is not None:
            difference=previous.rotation_difference(q).angle
            limited=previous.slerp(q,min(1,math.radians(6)/max(difference,1e-6)))
            ne=shoulder+limited@(elbow-shoulder);nw=shoulder+limited@(wrist-shoulder);np=limited@palm
            if score(shoulder,ne,nw,np,radius,colliders)<1e-8:
                return ne,nw,np,limited,initial,0.0,math.degrees(limited.angle)
        return e,w,p,q,initial,value,degrees
    l1=(elbow-shoulder).length; l2=(wrist-elbow).length
    guide=e.copy(); target=w.copy()
    for _ in range(24):
        guide,_=project(e,colliders,radius)
        target,_=project(w,colliders,radius*0.7)
        projected,_=project(w+p,colliders,radius*0.8)
        target += projected-(w+p)
        mid=e.lerp(w,0.5)
        projected,_=project(mid,colliders,radius*0.8)
        target += (projected-mid)*1.5
        ne,nw=two_bone(shoulder,target,guide,l1,l2,outward)
        candidate=score(shoulder,ne,nw,p,radius,colliders)
        if candidate < value:
            value,e,w=candidate,ne,nw
        else:
            break
        if value < 1e-8: break
    if value>1e-8:
        # A folded hand may be unreachable by rotating the whole arm. Give IK
        # an outward, reachable endpoint rather than preserving that fold.
        up=forward.cross(outward).normalized()
        safe=[]
        preferred=(wrist-shoulder).normalized()
        guide,_=project(elbow,colliders,radius*0.7)
        for direction in (preferred,forward,(forward-outward*0.5).normalized(),
                          (forward+outward*0.5).normalized(),outward,(outward+forward*0.6).normalized(),
                          (outward-forward*0.6).normalized(),
                          (outward+up*0.6).normalized(),(outward-up*0.6).normalized()):
            for reach in (0.98,0.90,0.8,0.6,0.4):
                target=shoulder+direction*(l1+l2)*reach
                ne,nw=two_bone(shoulder,target,guide,l1,l2,outward)
                hand_delta=palm.rotation_difference(direction*palm.length) if palm.length>1e-6 else Quaternion()
                np=hand_delta@palm
                candidate=score(shoulder,ne,nw,np,radius,colliders)
                if candidate<1e-8:
                    cost=(ne-elbow).length_squared+(nw-wrist).length_squared
                    safe.append((cost,ne,nw,np,hand_delta))
        if safe:
            _,e,w,p,q=min(safe,key=lambda candidate:candidate[0])
            value=0.0;degrees=math.degrees(q.angle)
    return e,w,p,q,initial,value,degrees
