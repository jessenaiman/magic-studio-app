# Blender 5.3: render a contact sheet of a rigged character's core actions (4 frames each).
# blender-5.3 -b --python action_sheet.py -- CHARACTER.glb OUT_DIR [jump_scale] [body_scale]
#   jump_scale: multiplies the hips' vertical travel in the jump clip (1.0 = as made)
#   body_scale: uniform size of the character (1.0 = as made)
import bpy, sys, os, math
from mathutils import Vector

args = sys.argv[sys.argv.index('--') + 1:]
SRC, OUT = args[0], args[1]
JUMP = float(args[2]) if len(args) > 2 else 1.0
BODY = float(args[3]) if len(args) > 3 else 1.0
VIDEO = len(args) > 4 and args[4] == 'video'   # every 2nd frame of each clip, for a watchable reel
os.makedirs(OUT, exist_ok=True)

# button/core action -> clip names to look for (first found)
CORE = [('idle', ['idle', 'happy-idle']), ('walk', ['walk']), ('jump', ['jump']), ('turn', ['turning', 'spin-in-place']),
        ('wave', ['waving', 'wave_goodbye_01']), ('clap', ['clapping']), ('sway', ['dancing', 'happy-idle']),
        ('sit', ['sitting-idle']), ('sleep', ['sleeping-idle']), ('bow', ['quick-formal-bow'])]


def fcurves(action):
    if hasattr(action, 'fcurves'):
        return list(action.fcurves)
    return [fc for l in action.layers for s in l.strips for b in s.channelbags for fc in b.fcurves]


for o in list(bpy.data.objects): bpy.data.objects.remove(o)
bpy.ops.import_scene.gltf(filepath=SRC)
arm = next(o for o in bpy.context.scene.objects if o.type == 'ARMATURE')
meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
arm.animation_data_create()
for t in list(arm.animation_data.nla_tracks): arm.animation_data.nla_tracks.remove(t)
clips = {a.name.split('|')[-1].rsplit('.', 1)[0] if a.name[-4:-3] == '.' else a.name.split('|')[-1]: a for a in bpy.data.actions}

# tweak 1: bigger/smaller jump = scale the hips' vertical location keys of the jump clip
if JUMP != 1.0 and 'jump' in clips:
    for fc in fcurves(clips['jump']):
        if 'Hips' in fc.data_path and fc.data_path.endswith('location'):
            base = fc.keyframe_points[0].co.y if len(fc.keyframe_points) else 0
            for k in fc.keyframe_points:
                for p in (k.co, k.handle_left, k.handle_right): p.y = base + (p.y - base) * JUMP
# tweak 2: body size
root = arm.parent or arm
root.scale = [s * BODY for s in root.scale]
bpy.context.view_layer.update()

# camera + light framed on the rest-pose bounds (same setup as the felt-shader renders)
pts = [m.matrix_world @ v.co for m in meshes for i, v in enumerate(m.data.vertices) if i % 20 == 0]
lo = Vector([min(p[i] for p in pts) for i in range(3)]); hi = Vector([max(p[i] for p in pts) for i in range(3)])
c, h = (lo + hi) / 2, (hi - lo).z * 1.6
sc = bpy.context.scene
for eng in ('BLENDER_EEVEE_NEXT', 'BLENDER_EEVEE'):
    try: sc.render.engine = eng; break
    except TypeError: pass
sc.render.resolution_x, sc.render.resolution_y = 240, 320
cam = bpy.data.objects.new('Cam', bpy.data.cameras.new('Cam')); sc.collection.objects.link(cam); sc.camera = cam
cam.location = (c.x, c.y - h * 1.5, c.z + h * 0.05); cam.rotation_euler = (math.radians(89), 0, 0)
key = bpy.data.objects.new('Key', bpy.data.lights.new('Key', 'AREA')); sc.collection.objects.link(key)
key.data.energy, key.data.size = 500 * h * h, h; key.location = (c.x + h, c.y - h * 1.5, c.z + h); key.rotation_euler = (math.radians(55), 0, math.radians(35))
sc.world = sc.world or bpy.data.worlds.new('W'); sc.world.use_nodes = True
bg = next(n for n in sc.world.node_tree.nodes if n.type == 'BACKGROUND'); bg.inputs['Color'].default_value = (0.75, 0.78, 0.82, 1)

rows = []
for label, wants in CORE:
    act = next((clips[w] for w in wants if w in clips), None)
    if not act: rows.append((label, None)); print('MISSING', label); continue
    arm.animation_data.action = act
    if hasattr(arm.animation_data, 'action_slot') and act.slots: arm.animation_data.action_slot = act.slots[0]
    f0, f1 = map(int, act.frame_range)
    paths = []
    steps = range(f0, f1, 2) if VIDEO else [f0 + (f1 - f0) * i // 4 for i in range(4)]
    for i, f in enumerate(steps):
        sc.frame_set(f)
        p = os.path.join(OUT, f'{label}_{i:03d}.png' if VIDEO else f'{label}_{i}.png'); sc.render.filepath = p
        bpy.ops.render.render(write_still=True); paths.append(p)
    rows.append((label, paths))
print('SHEET', [(l, bool(p)) for l, p in rows])
