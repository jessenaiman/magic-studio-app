"""Render a 3D model's turnaround as layout guides for asset sheets (Blender, headless).

    blender --background --python pipeline/model_turnaround_guides.py -- MODEL.glb OUT_DIR [SIZE]

Writes one transparent orthographic PNG per view (front, three-quarter, side, back),
all at one scale with the feet on one baseline, plus turnaround.png side by side.
"""

import math
import sys
from pathlib import Path

import bpy
from mathutils import Vector

VIEWS = {"front": 0, "three_quarter": 45, "side": 90, "back": 180}


def import_model(path: Path) -> list:
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=str(path))
    return [o for o in bpy.context.scene.objects if o.type == "MESH"]


def bounds(meshes: list) -> tuple:
    corners = [o.matrix_world @ Vector(c) for o in meshes for c in o.bound_box]
    low = Vector(min(c[i] for c in corners) for i in range(3))
    high = Vector(max(c[i] for c in corners) for i in range(3))
    return low, high


def setup_scene(size: int, scale: float):
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.color_type = "TEXTURE"
    scene.display.shading.light = "FLAT"
    scene.render.film_transparent = True
    scene.render.resolution_x = scene.render.resolution_y = size
    camera = bpy.data.objects.new("guide_camera", bpy.data.cameras.new("guide_camera"))
    camera.data.type = "ORTHO"
    camera.data.ortho_scale = scale
    scene.collection.objects.link(camera)
    scene.camera = camera
    return scene, camera


def render_views(meshes: list, out: Path, size: int) -> list:
    low, high = bounds(meshes)
    centre = (low + high) / 2
    radius = max(high - low) * 2
    scene, camera = setup_scene(size, max(high - low) * 1.1)
    paths = []
    for name, degrees in VIEWS.items():
        angle = math.radians(degrees)
        # glTF front faces +Y after import (Blender Z up); orbit around the vertical axis.
        camera.location = centre + Vector((math.sin(angle) * radius, -math.cos(angle) * radius, 0))
        camera.rotation_euler = (math.radians(90), 0, angle)
        scene.render.filepath = str(out / f"{name}.png")
        bpy.ops.render.render(write_still=True)
        paths.append(out / f"{name}.png")
    return paths


def side_by_side(paths: list, out: Path, size: int) -> Path:
    sheet = bpy.data.images.new("turnaround", size * len(paths), size, alpha=True)
    pixels = [0.0] * (size * len(paths) * size * 4)
    for column, path in enumerate(paths):
        view = bpy.data.images.load(str(path))
        source = list(view.pixels)
        for row in range(size):
            start = (row * size * len(paths) + column * size) * 4
            pixels[start : start + size * 4] = source[row * size * 4 : (row + 1) * size * 4]
    sheet.pixels = pixels
    sheet.filepath_raw = str(out / "turnaround.png")
    sheet.file_format = "PNG"
    sheet.save()
    return out / "turnaround.png"


def main() -> None:
    args = sys.argv[sys.argv.index("--") + 1 :]
    model, out = Path(args[0]), Path(args[1])
    size = int(args[2]) if len(args) > 2 else 768
    out.mkdir(parents=True, exist_ok=True)
    paths = render_views(import_model(model), out, size)
    print(side_by_side(paths, out, size))


main()
