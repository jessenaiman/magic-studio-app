"""Continuous position constraints for a shoulder/elbow/wrist chain."""
import math
from mathutils import Vector, Quaternion
from arm_collisions import project, score, two_bone


def directional_project(point,colliders,radius,direction):
    """Keep a consistent contact side instead of jumping across a body's center."""
    result=point.copy();worst=0.0;direction=direction.normalized()
    for center,axes,radii in colliders:
        p=axes.transposed()@(result-center);d=axes.transposed()@direction
        r=radii+Vector((radius,radius,radius))
        c=sum((p[i]/r[i])**2 for i in range(3))-1
        if c>=0:continue
        a=sum((d[i]/r[i])**2 for i in range(3))
        b=2*sum(p[i]*d[i]/r[i]**2 for i in range(3))
        t=(-b+math.sqrt(max(0,b*b-4*a*c)))/(2*a)
        result+=direction*(t+min(r)*0.003);worst=max(worst,-c)
    return result,worst


def solve(shoulder, elbow, wrist, palm, radius, colliders, forward, outward, previous=None):
    before=score(shoulder,elbow,wrist,palm,radius,colliders)
    l1=(elbow-shoulder).length;l2=(wrist-elbow).length
    e=elbow.copy();w=wrist.copy()
    bias=previous.get('bias',Vector()) if isinstance(previous,dict) else Vector()
    w+=bias
    if isinstance(previous,dict) and 'elbow' in previous:
        e+=(previous['elbow']-previous['sourceElbow'])*0.85
        w+=(previous['wrist']-previous['sourceWrist'])*0.85
    if before<1e-10 and (e-elbow).length+(w-wrist).length<1e-5:
        return elbow,wrist,palm,Quaternion(),before,before,0.0
    for _ in range(140):
        # Alternating length and body constraints converges without choosing a
        # different quaternion branch or rotating the fingers away from the pose.
        d=e-shoulder
        e=shoulder+(d.normalized() if d.length>1e-8 else outward.normalized())*l1
        d=w-e
        if d.length>1e-8:
            correction=d*(1-l2/d.length)
            e+=correction*0.45;w-=correction*0.55
        e,_=project(e,colliders,radius*0.7)
        w,_=project(w,colliders,radius*0.7)
        projected,_=project(w+palm,colliders,radius*0.8)
        w+=projected-(w+palm)
        mid=e.lerp(w,0.5);projected,_=project(mid,colliders,radius*0.8)
        push=projected-mid;e+=push*0.7;w+=push*1.3
    # Restore exact bone lengths, then repeat at the reachable endpoint.
    for _ in range(32):
        e,w=two_bone(shoulder,w,e,l1,l2,outward)
        value=score(shoulder,e,w,palm,radius,colliders)
        if value<1e-8:break
        pe,_=project(e,colliders,radius*0.7)
        pw,_=project(w,colliders,radius*0.7)
        pp,_=project(w+palm,colliders,radius*0.8)
        w=pw+(pp-(w+palm));e=pe
    e,w=two_bone(shoulder,w,e,l1,l2,outward)
    after=score(shoulder,e,w,palm,radius,colliders)
    if isinstance(previous,dict) and 'elbow' in previous:
        old_e=previous['elbow']+(shoulder-previous['shoulder'])
        alt_e,alt_w=two_bone(shoulder,w,old_e,l1,l2,outward)
        alt_score=score(shoulder,alt_e,alt_w,palm,radius,colliders)
        if alt_score<1e-8 and (alt_e-old_e).length_squared<(e-old_e).length_squared:
            e,w,after=alt_e,alt_w,alt_score
    swivel=0.0
    if after>1e-8 and (w-shoulder).length>1e-6:
        axis=(w-shoulder).normalized()
        # Elbow swivel is the IK redundancy that clears a sleeve without
        # changing the wrist target or either arm-bone length.
        for degrees in range(3,61,3):
            safe=[]
            for sign in (1,-1):
                angle=math.radians(degrees*sign)
                ne=shoulder+Quaternion(axis,angle)@(e-shoulder)
                if score(shoulder,ne,w,palm,radius,colliders)<1e-8:
                    lo=0.0;hi=angle
                    for _ in range(12):
                        mid=(lo+hi)/2;candidate=shoulder+Quaternion(axis,mid)@(e-shoulder)
                        if score(shoulder,candidate,w,palm,radius,colliders)<1e-8:hi=mid
                        else:lo=mid
                    candidate=shoulder+Quaternion(axis,hi)@(e-shoulder)
                    safe.append((candidate.dot(outward),candidate,abs(math.degrees(hi))))
            if safe:
                _,e,swivel=max(safe,key=lambda x:x[0]);after=score(shoulder,e,w,palm,radius,colliders);break
    if after>1e-8:
        # At full extension the swivel circle collapses. A tiny reachable wrist
        # adjustment restores elbow freedom without stretching the chain.
        safe=[]
        for reach in (0.995,0.99,0.98,0.97,0.95,0.93,0.9,0.85,0.8):
            for shift in range(0,101):
                goal=w+outward*(shift*(l1+l2)*0.008)
                d=goal-shoulder
                if d.length<1e-6:continue
                goal=shoulder+d.normalized()*min(d.length,(l1+l2)*reach)
                for guide in (e+outward*(l1*0.5),shoulder+outward*l1):
                    ne,nw=two_bone(shoulder,goal,guide,l1,l2,outward)
                    if score(shoulder,ne,nw,palm,radius,colliders)<1e-8:
                        cost=(nw-wrist).length_squared+(ne-elbow).length_squared*0.5
                        if isinstance(previous,dict) and 'elbow' in previous:
                            old_e=previous['elbow']+(shoulder-previous['shoulder'])
                            cost+=(ne-old_e).length_squared*0.25
                        safe.append((cost,ne,nw))
        if safe:
            _,e,w=min(safe,key=lambda x:x[0]);after=score(shoulder,e,w,palm,radius,colliders)
    return e,w,palm,Quaternion(),before,after,swivel
