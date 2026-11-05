# Blender 5.3 helper for build_catalog.py: import one model, report rig + animations, render a thumbnail.
# blender-5.3 -b --python catalog_probe.py -- MODEL OUT.png
import bpy, sys, json, math

model, out = sys.argv[sys.argv.index('--') + 1:][:2]
for o in list(bpy.data.objects): bpy.data.objects.remove(o)
ext = model.lower().rsplit('.', 1)[-1]
if ext == 'fbx': bpy.ops.import_scene.fbx(filepath=model)
else: bpy.ops.import_scene.gltf(filepath=model)

arm = next((o for o in bpy.context.scene.objects if o.type == 'ARMATURE'), None)
rig = None
if arm:
    names = [b.name for b in arm.data.bones]
    rig = 'mixamo' if any(n.startswith('mixamorig') for n in names) else f'custom:{len(names)}'
anims = sorted({a.name.split('|')[-1].rsplit('.', 1)[0] for a in bpy.data.actions})
if arm and arm.animation_data: arm.animation_data.action = None

meshes = [o for o in bpy.context.scene.objects if o.type == 'MESH']
pts = [o.matrix_world @ v.co for o in meshes for i, v in enumerate(o.data.vertices) if i % max(1, len(o.data.vertices) // 4000) == 0]
lo = [min(p[i] for p in pts) for i in range(3)]; hi = [max(p[i] for p in pts) for i in range(3)]
c = [(lo[i] + hi[i]) / 2 for i in range(3)]; size = max(hi[i] - lo[i] for i in range(3))
sc = bpy.context.scene
for eng in ('BLENDER_EEVEE_NEXT', 'BLENDER_EEVEE'):
    try: sc.render.engine = eng; break
    except TypeError: pass
sc.render.resolution_x = sc.render.resolution_y = 384
sc.render.film_transparent = True
cam = bpy.data.objects.new('Cam', bpy.data.cameras.new('Cam')); sc.collection.objects.link(cam); sc.camera = cam
d = size * 2.2; yaw = math.radians(30)
cam.location = (c[0] + d * math.sin(yaw), c[1] - d * math.cos(yaw), c[2] + size * 0.25)
cam.rotation_euler = (math.radians(82), 0, yaw)
sun = bpy.data.objects.new('Sun', bpy.data.lights.new('Sun', 'SUN')); sc.collection.objects.link(sun)
sun.data.energy = 3; sun.rotation_euler = (math.radians(50), 0, math.radians(30))
sc.world = sc.world or bpy.data.worlds.new('W'); sc.world.use_nodes = True
bg = next(n for n in sc.world.node_tree.nodes if n.type == 'BACKGROUND'); bg.inputs['Color'].default_value = (0.6, 0.6, 0.6, 1); bg.inputs['Strength'].default_value = 0.6
sc.render.filepath = out
try: bpy.ops.render.render(write_still=True)
except Exception as e: print('render failed', e)
print('PROBE ' + json.dumps({'rig': rig, 'animations': anims}))
