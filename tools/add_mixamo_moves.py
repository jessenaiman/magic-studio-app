# Blender 5.3: retarget, condition measured skin tears, and bake arm/body clearance.
# blender-5.3 -b --factory-startup --python-exit-code 1 --python tools/add_mixamo_moves.py -- CHARACTER.(fbx|glb) OUT.glb [PREVIEW.png MOVE]
# Collected source files are never overwritten. Existing outputs are backed up.
import bpy, sys, os, math, json, runpy, shutil, tempfile
from pathlib import Path
from datetime import datetime, timezone

args=sys.argv[sys.argv.index('--')+1:]
source,output=map(lambda p:Path(p).resolve(),args[:2])
preview,preview_move=(args[2],args[3]) if len(args)>3 else (None,None)
if source==output:raise ValueError('Use a separate output file to preserve the collected source.')
ROOT=Path(__file__).resolve().parents[1]
LIB=ROOT/'tools/mixamo'
original_argv=sys.argv[:]

def run(name,arguments):
    sys.argv=[str(LIB/name),'--',*map(str,arguments)]
    try:return runpy.run_path(str(LIB/name),run_name='__main__')
    finally:sys.argv=original_argv[:]

with tempfile.TemporaryDirectory(prefix='mixamo-rig-') as scratch:
    scratch=Path(scratch)
    combined=scratch/'combined.glb'
    run('retarget.py',[source,combined])
    run('check_retarget.py',[combined,ROOT/'assets/animations/mixamo','*'])
    candidate=scratch/'corrected.glb'
    run('pbd_bake.py',[combined,candidate])
    run('verify_collision.py',[candidate])
    for attempt in range(2):
        report=json.loads(candidate.with_suffix('.review.json').read_text())
        flagged=[row['clip'] for row in report if row['contactArmFrames']]
        if not flagged:break
        retry=scratch/f'corrected-{attempt+1}.glb'
        run('pbd_bake.py',[candidate,retry,','.join(flagged)])
        candidate=retry
        run('verify_collision.py',[candidate])
    try:
        run('verify_deformation.py',[candidate])
    except AssertionError:
        conditioned=scratch/'conditioned.glb'
        run('condition_weights.py',[candidate,conditioned])
        candidate=conditioned
        run('verify_collision.py',[candidate])
        run('verify_deformation.py',[candidate])
    # Match both hands to the clap, then restore body clearance; support the
    # sleeping pose using the character's actual head/skin dimensions.
    hand_fit=scratch/'hand-fit.glb'
    run('repair_clap.py',[candidate,hand_fit])
    hand_clear=scratch/'hand-clear.glb'
    run('pbd_bake.py',[hand_fit,hand_clear,'clapping'])
    candidate=scratch/'grounded.glb'
    run('ground_sleep.py',[hand_clear,candidate])
    run('verify_collision.py',[candidate])
    run('verify_full_deformation.py',[candidate,candidate.with_suffix('.collision.json'),candidate.with_suffix('.deformation.json')])
    report=json.loads(candidate.with_suffix('.review.json').read_text())
    baseline=scratch/'baseline.review.json'
    run('verify_collision.py',[combined,candidate.with_suffix('.collision.json'),baseline])
    before={row['clip']:row for row in json.loads(baseline.read_text())}
    contacts=sum(row['contactArmFrames'] for row in report)
    jumps=[row['clip'] for row in report if row['maxArmStepDegrees']>before[row['clip']]['maxArmStepDegrees']*1.5+12]
    if contacts or jumps:
        review=ROOT/'media/mixamo-unreviewed'/output.name
        review.parent.mkdir(parents=True,exist_ok=True)
        shutil.copy2(candidate,review)
        for suffix in ('.review.json','.collision.json','.deformation.json'):shutil.copy2(candidate.with_suffix(suffix),review.with_suffix(suffix))
        raise RuntimeError(f'Output retained for individual review: {review}; contact frames={contacts}; motion jumps={jumps}')
    output.parent.mkdir(parents=True,exist_ok=True)
    if output.exists():
        backup=ROOT/'media/mixamo-backups'/datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ')/output.name
        backup.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(output,backup)
    pending=output.with_name(output.stem+'.pending.glb')
    shutil.copy2(candidate,pending);os.replace(pending,output)
    for suffix in ('.review.json','.collision.json','.deformation.json'):shutil.copy2(candidate.with_suffix(suffix),output.with_suffix(suffix))
    if candidate.with_suffix('.grounding.json').exists():shutil.copy2(candidate.with_suffix('.grounding.json'),output.with_suffix('.grounding.json'))
    if candidate.with_suffix('.conditioning.json').exists():shutil.copy2(candidate.with_suffix('.conditioning.json'),output.with_suffix('.conditioning.json'))
    print('RIGGED_EXPORT',output,'clips',len(report),'contact_frames',contacts,flush=True)

# Reimport the accepted export so the optional preview shows the repaired rig.
if preview:
    bpy.ops.object.select_all(action='SELECT');bpy.ops.object.delete(use_global=False)
    for action in list(bpy.data.actions):bpy.data.actions.remove(action)
    bpy.ops.import_scene.gltf(filepath=str(output))
# Preserve the earlier script's optional five-frame preview interface.
arm=next(o for o in bpy.context.scene.objects if o.type=='ARMATURE')
char=[o for o in bpy.context.scene.objects if o.type=='ARMATURE' or (o.type=='MESH' and any(m.type=='ARMATURE' and m.object==arm for m in o.modifiers))]

if preview:                                      # frame strip of one move, for a visual check
    act = bpy.data.actions[preview_move]
    for t in arm.animation_data.nla_tracks: t.mute = True
    arm.animation_data.action = act
    if hasattr(arm.animation_data, 'action_slot') and act.slots:
        arm.animation_data.action_slot = act.slots[0]
    sc = bpy.context.scene
    for eng in ('BLENDER_EEVEE_NEXT', 'BLENDER_EEVEE'):
        try: sc.render.engine = eng; break
        except TypeError: pass
    sc.render.resolution_x, sc.render.resolution_y = 320, 400
    mesh = next(o for o in char if o.type == 'MESH')
    h = max((mesh.matrix_world @ v.co).z for i, v in enumerate(mesh.data.vertices) if i % 50 == 0)
    cam = bpy.data.objects.new('Cam', bpy.data.cameras.new('Cam')); sc.collection.objects.link(cam); sc.camera = cam
    cam.location = (0, -h * 3.2, h * 0.55); cam.rotation_euler = (math.radians(88), 0, 0)
    sun = bpy.data.objects.new('Sun', bpy.data.lights.new('Sun', 'SUN')); sc.collection.objects.link(sun)
    sun.data.energy = 3; sun.rotation_euler = (math.radians(45), 0, math.radians(20))
    f0, f1 = map(int, act.frame_range)
    frames = [f0 + (f1 - f0) * i // 4 for i in range(5)]
    paths = []
    for i, f in enumerate(frames):
        sc.frame_set(f); p = f'{preview}.{i}.png'; sc.render.filepath = p
        bpy.ops.render.render(write_still=True); paths.append(p)
    print('PREVIEW', paths)
