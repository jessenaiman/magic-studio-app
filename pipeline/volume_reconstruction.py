"""Reconstruct a volume from registered transparent views, independent of character presets."""
from __future__ import annotations

import json
import math
import hashlib
from pathlib import Path

def _crop_sheet_view(spec: dict, view: dict, vi: int):
    import numpy as np
    from PIL import Image
    from scipy import ndimage

    sheet = Image.open(view["sheet"]).convert("RGBA")
    sheet_rgba = np.array(sheet)
    components, _ = ndimage.label(sheet_rgba[..., 3] >= 128)
    sizes = np.bincount(components.ravel())
    sizes[0] = 0
    ids = sorted(
        np.argsort(sizes)[-len(spec["views"]) :],
        key=lambda k: float(np.where(components == k)[1].mean()),
    )
    if len(ids) != len(spec["views"]) or min(sizes[k] for k in ids) < 1000:
        raise ValueError(
            f"{spec['slug']}: sheet must contain one separate full-body figure per view"
        )
    component = components == ids[vi]
    _, xx0 = np.where(component)
    sheet_rgba[..., 3] = np.where(component, sheet_rgba[..., 3], 0)
    return Image.fromarray(sheet_rgba).crop(
        (
            max(0, int(xx0.min()) - 4),
            0,
            min(sheet.width, int(xx0.max()) + 5),
            sheet.height,
        )
    )


def _view_landmarks(view: dict, mask, yy, xx):
    import numpy as np

    crown, baseline = int(yy.min()), int(yy.max()) + 1
    if "sheet" in view:
        foot = mask[max(crown, baseline - max(2, (baseline - crown) // 10)) : baseline]
        fx = np.where(foot)[1]
        center = (
            (float(fx.min()) + float(fx.max())) / 2 if len(fx) else float(np.median(xx))
        )
    else:
        crown, baseline, center = view["crown"], view["baseline"], view["center"]
    if not all(math.isfinite(v) for v in (crown, baseline, center)) or baseline <= crown:
        raise ValueError(f"{view['label']}: invalid registered crown, baseline or center")
    return crown, baseline, center


def _prepare_view_texture(spec: dict, view: dict, vi: int, height: float, out_dir: Path) -> dict:
    import numpy as np
    from PIL import Image
    from scipy import ndimage

    image = (
        _crop_sheet_view(spec, view, vi)
        if "sheet" in view
        else Image.open(view["file"]).convert("RGBA")
    )
    rgba = np.array(image)
    mask = rgba[..., 3] >= 128
    if mask.all():
        raise ValueError(f"{view['label']}: remove the background before volume reconstruction")
    _, count = ndimage.label(mask)
    if count == 0:
        raise ValueError(f"{spec['slug']} {view['label']}: empty silhouette")
    yy, xx = np.where(mask)
    crown, baseline, center = _view_landmarks(view, mask, yy, xx)
    scale = (baseline - crown) / height
    eroded = ndimage.binary_erosion(mask, iterations=2)
    color_src = eroded if int(eroded.sum()) >= 100 else mask
    nearest = ndimage.distance_transform_edt(
        ~color_src, return_distances=False, return_indices=True
    )
    rgb = rgba[..., :3][tuple(nearest)]
    path = out_dir / f"{vi:02d}_projection.png"
    Image.fromarray(rgb.astype("uint8")).save(path)
    src_path = Path(view.get("file") or view["sheet"])
    return dict(
        view,
        texture=str(path.resolve()),
        center=center,
        baseline=baseline,
        pixels_per_m=scale,
        source_sha256=hashlib.sha256(src_path.read_bytes()).hexdigest(),
        size=list(image.size),
        silhouette=mask,
        distance=ndimage.distance_transform_edt(mask)
        - ndimage.distance_transform_edt(~mask),
    )


def _build_voxel_axes(textures: list[dict], height: float, resolution: int):
    import numpy as np

    front = next(view for view in textures if float(view["angle"]) % 360 == 0)
    _, fx = np.where(front["silhouette"])
    half_x = max(abs(fx - front["center"])) / front["pixels_per_m"] + height * 0.025
    profile = next(v for v in textures if float(v["angle"]) % 360 in (90, 270))
    _, px = np.where(profile["silhouette"])
    half_y = max(abs(px - profile["center"])) / profile["pixels_per_m"] + height * 0.025
    step = height / resolution
    axes = [
        np.arange(-half_x, half_x + step, step, dtype=np.float32),
        np.arange(-half_y, half_y + step, step, dtype=np.float32),
        np.arange(-step * 2, height + step * 3, step, dtype=np.float32),
    ]
    if math.prod(len(axis) for axis in axes) > 32_000_000:
        raise ValueError(
            "Registered view extents exceed 32 million voxels; check scale or lower resolution"
        )
    return axes, step


def _carve_occupied_voxels(spec: dict, textures: list[dict], axes: list, step: float):
    import numpy as np
    from scipy import ndimage

    x, y, z = np.meshgrid(*axes, indexing="ij")
    field = np.full(x.shape, 100.0, dtype=np.float32)
    for view in textures:
        angle = math.radians(view["angle"])
        u = (
            view["center"]
            + (x * math.cos(angle) + y * math.sin(angle)) * view["pixels_per_m"]
        )
        v = view["baseline"] - z * view["pixels_per_m"]
        sdf = ndimage.map_coordinates(
            view["distance"], [v, u], order=1, mode="constant", cval=-10000
        )
        field = np.minimum(field, sdf / view["pixels_per_m"])
    occupied = ndimage.gaussian_filter(field, 0.65) > -step * 0.35
    occupied[[0, -1], :, :] = False
    occupied[:, [0, -1], :] = False
    occupied[:, :, [0, -1]] = False
    _, component_count = ndimage.label(occupied)
    if occupied.sum() < 100:
        raise ValueError(
            f"{spec['slug']}: supplied views do not produce a usable common volume"
        )
    return occupied, int(component_count)


def _extract_voxel_quad_mesh(occupied, axes: list, step: float):
    import numpy as np

    face_corners = [
        [(0, 0, 0), (0, 0, 1), (0, 1, 1), (0, 1, 0)],
        [(1, 0, 0), (1, 1, 0), (1, 1, 1), (1, 0, 1)],
        [(0, 0, 0), (1, 0, 0), (1, 0, 1), (0, 0, 1)],
        [(0, 1, 0), (0, 1, 1), (1, 1, 1), (1, 1, 0)],
        [(0, 0, 0), (0, 1, 0), (1, 1, 0), (1, 0, 0)],
        [(0, 0, 1), (1, 0, 1), (1, 1, 1), (0, 1, 1)],
    ]
    faces = []
    for axis in range(3):
        for positive in [False, True]:
            neighbor = np.roll(occupied, -1 if positive else 1, axis=axis)
            cells = np.argwhere(occupied & ~neighbor)
            faces.append(
                cells[:, None, :]
                + np.array(face_corners[axis * 2 + positive])[None, :, :]
            )
    corners = np.concatenate(faces).reshape(-1, 3)
    unique, inverse = np.unique(corners, axis=0, return_inverse=True)
    vertices = unique.astype(np.float32) * step + np.array(
        [a[0] - step / 2 for a in axes]
    )
    quads = inverse.reshape(-1, 4)
    return vertices.astype(np.float32), quads.astype(np.int32)


def prepare_volume(spec: dict, out_dir: Path, resolution: int = 144) -> dict:
    """Intersect the supplied silhouette cones; retain real image projections on the closed hull."""
    import numpy as np

    height = float(spec["height_m"])
    if not math.isfinite(height) or height <= 0 or not 16 <= resolution <= 256:
        raise ValueError("height_m must be positive; resolution must be 16..256")
    views = spec["views"]
    angles = [float(view["angle"]) % 360 for view in views]
    if 0 not in angles or not any(angle in (90, 270) for angle in angles):
        raise ValueError("Volume reconstruction requires a front view and an orthogonal profile")
    out_dir.mkdir(parents=True, exist_ok=True)
    textures = [
        _prepare_view_texture(spec, view, vi, height, out_dir)
        for vi, view in enumerate(views)
    ]
    axes, step = _build_voxel_axes(textures, height, resolution)
    occupied, component_count = _carve_occupied_voxels(spec, textures, axes, step)
    vertices, quads = _extract_voxel_quad_mesh(occupied, axes, step)
    mesh_path = out_dir / "visual_hull.npz"
    np.savez_compressed(mesh_path, vertices=vertices, faces=quads)
    clean_views = [
        {k: v for k, v in t.items() if k not in ("silhouette", "distance")}
        for t in textures
    ]
    geom_hash = hashlib.sha256(vertices.tobytes() + quads.tobytes()).hexdigest()
    spec = dict(
        spec,
        views=clean_views,
        mesh_file=str(mesh_path.resolve()),
        output_dir=str(out_dir.resolve()),
        reconstruction=dict(
            method="registered_silhouette_intersection",
            view_count=len(textures),
            connected_components=component_count,
            resolution=resolution,
            vertices=len(vertices),
            faces=len(quads),
            rigged=False,
            geometry_sha256=geom_hash,
        ),
    )
    (out_dir / "asset_sources.json").write_text(json.dumps(spec, indent=2) + "\n")
    return spec


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--spec", type=Path, required=True)
    parser.add_argument("--out-dir", type=Path, required=True)
    parser.add_argument("--resolution", type=int, default=144)
    args = parser.parse_args()
    spec = json.loads(args.spec.read_text())
    for view in spec["views"]:
        for key in ("file", "sheet"):
            if key in view:
                path = Path(view[key])
                view[key] = str(path if path.is_absolute() else args.spec.resolve().parent / path)
    result = prepare_volume(spec, args.out_dir.resolve(), args.resolution)
    print(json.dumps({"ok": True, "mesh": result["mesh_file"],
                      "sources": str(args.out_dir.resolve() / "asset_sources.json"),
                      "reconstruction": result["reconstruction"]}))
