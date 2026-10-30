"""Build a textured, Z-up volume and inspection turntable in Blender."""
import json
import math
from pathlib import Path


def build(spec, duration, output, track="turntable"):
    import bpy
    import numpy as np

    scene = bpy.data.scenes.new(spec["slug"] + "-volume")
    bpy.context.window.scene = scene
    data = np.load(spec["mesh_file"])
    mesh = bpy.data.meshes.new(spec["slug"])
    mesh.from_pydata(data["vertices"].tolist(), [], data["faces"].tolist())
    mesh.update()
    obj = bpy.data.objects.new(spec["slug"], mesh)
    scene.collection.objects.link(obj)
    views = spec["views"]
    directions = []
    for view in views:
        angle = math.radians(view["angle"])
        directions.append((math.sin(angle), -math.cos(angle), 0))
        material = bpy.data.materials.new(view["label"])
        material.use_nodes = True
        nodes, links = material.node_tree.nodes, material.node_tree.links
        shader = nodes.get("Principled BSDF")
        texture = nodes.new("ShaderNodeTexImage")
        texture.image = bpy.data.images.load(view["texture"], check_existing=True)
        texture.image.pack()
        links.new(texture.outputs["Color"], shader.inputs["Base Color"])
        links.new(texture.outputs["Color"], shader.inputs["Emission Color"])
        shader.inputs["Emission Strength"].default_value = 1
        shader.inputs["Roughness"].default_value = 1
        mesh.materials.append(material)
    uv = mesh.uv_layers.new()
    for face in mesh.polygons:
        index = max(range(len(views)), key=lambda i: face.normal.dot(directions[i]))
        face.material_index = index
        view = views[index]
        angle = math.radians(view["angle"])
        width, height = view["size"]
        for loop in face.loop_indices:
            point = mesh.vertices[mesh.loops[loop].vertex_index].co
            uv.data[loop].uv = (
                (view["center"] + (point.x * math.cos(angle) + point.y * math.sin(angle)) * view["pixels_per_m"]) / width,
                1 - (view["baseline"] - point.z * view["pixels_per_m"]) / height,
            )
    scene.render.fps = 24
    scene.frame_start, scene.frame_end = 1, int(duration * 24)
    rig = None
    if track == "turntable":
        for frame in range(1, scene.frame_end + 2):
            obj.rotation_euler.z = math.tau * (frame - 1) / scene.frame_end
            obj.keyframe_insert("rotation_euler", frame=frame)
    else:
        from bind_volume_skeleton import bind

        skeleton = spec["skeleton"]
        if skeleton.get("geometry_sha256") != spec["reconstruction"]["geometry_sha256"]:
            raise ValueError("Skeleton weights belong to different geometry; rebind the reconstructed volume")
        rig = bind(obj, skeleton, track, scene.render.fps)
    obj["reconstruction_sources"] = json.dumps(spec)
    obj["rigged"] = rig is not None
    vertices = data["vertices"]
    diameter = 2 * float(np.linalg.norm(vertices[:, :2], axis=1).max())
    low, high = float(vertices[:, 2].min()), float(vertices[:, 2].max())
    if rig:
        for frame in range(scene.frame_start, scene.frame_end + 1):
            scene.frame_set(frame)
            evaluated = obj.evaluated_get(bpy.context.evaluated_depsgraph_get())
            for vertex in evaluated.data.vertices:
                point = evaluated.matrix_world @ vertex.co
                diameter = max(diameter, 2 * math.hypot(point.x, point.y))
                low, high = min(low, point.z), max(high, point.z)
    extent = max(diameter, high - low) * 1.1
    camera = bpy.data.cameras.new("Volume inspection")
    camera.type, camera.ortho_scale = "ORTHO", extent
    camera.clip_start, camera.clip_end = extent / 1000, extent * 10
    view = bpy.data.objects.new("Volume inspection", camera)
    scene.collection.objects.link(view)
    view.location, view.rotation_euler = (0, -extent * 3, (low + high) / 2), (math.pi / 2, 0, 0)
    scene.camera = view
    scene.render.resolution_x = scene.render.resolution_y = 512
    scene.render.resolution_percentage = 100
    scene.render.film_transparent = True
    scene.view_settings.view_transform = "Standard"
    scene.cycles.samples = 16
    scene.frame_set(1)
    bpy.ops.object.select_all(action="DESELECT")
    obj.select_set(True)
    if rig:
        rig.select_set(True)
    bpy.context.view_layer.objects.active = obj
    bpy.ops.export_scene.gltf(filepath=str(Path(output) / "volume.glb"), export_format="GLB",
                              use_selection=True, use_active_scene=True)
