"""Animate any set of pre-separated transparent layers and export transparent video.

A spec describes layers (image, pivot, optional parent) and tracks (a duration plus motion per
layer). Nothing here assumes what the layers depict: no anatomy, no part names, no counts.
Motion is built from a few generic terms that add together per property:

  keys       [[t, value], ...]            value is a number, or [x, y] for position/scale
  ellipse    centre, rx, ry, start, turns, from, to     position offset along an ellipse
  oscillate  amplitude, hz, phase, axis, envelope       continuous sine (stays smooth under motion blur)
  follow     layer, gain, limit, smooth                 rotation leaning with a layer's horizontal velocity

Optional spec fields: `margin` [left, top, right, bottom] enlarges the clip beyond the layer canvas (room for
artwork that touches the edge or moves outside it; the layers keep their place, offset by left/top), `reference` an image of the expected look and track `loop` true, both used by the validation
that runs on every rendered clip.
`alternates` declares mutually exclusive layer pairs, e.g. [["eyes_open", "eyes_closed"]].
Use [] when overlapping layers depict separate objects. Omit it to infer alternates from overlap.

Every term is an offset from the resting state: position in pixels from the pivot, rotation in degrees,
scale in percentage points added to 100, opacity in percentage points added to 100 (so -100 hides).
Times are seconds from the start of the track.
The result is one transparent WebM per track, rendered by EffectCraft.
"""

import argparse
import json
import math
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from pipeline.motion_validate import analyze, contact_sheet, lint_spec  # noqa: E402

try:
    import numpy as np
    from PIL import Image
except ImportError as exc:
    sys.exit(f"layered_motion needs numpy and Pillow ({exc})")

try:
    from scipy import ndimage as _scipy_ndimage
except ImportError:
    _scipy_ndimage = None

PROPS = {"position": 2, "rotation": 1, "scale": 2, "opacity": 1}
EASES = {
    "hold": lambda u: 0 if u < 1 else 1,
    "linear": lambda u: u,
    "smooth": lambda u: u * u * (3 - 2 * u),
    "in": lambda u: u * u * u,
    "out": lambda u: 1 - (1 - u) ** 3,
}
RESOLUTIONS = {"full", "half", "third", "quarter"}
QUALITIES = {"best", "draft"}
NVIDIA_VULKAN_ICD = Path("/usr/share/vulkan/icd.d/nvidia_icd.json")


def number(value, what):
    if (
        not isinstance(value, (int, float))
        or isinstance(value, bool)
        or not math.isfinite(value)
    ):
        raise ValueError(f"{what} must be a finite number, got {value!r}")
    return float(value)


def vector(value, size, what):
    items = value if isinstance(value, list) else [value] * size
    if len(items) != size:
        raise ValueError(f"{what} needs {size} number(s)")
    return [number(v, what) for v in items]


def ease(name, u):
    if name not in EASES:
        raise ValueError(f"Unknown ease {name!r}; use one of {sorted(EASES)}")
    return EASES[name](max(0.0, min(1.0, u)))


def alpha_bounds(path):
    with Image.open(path) as image:
        box = (
            image.convert("RGBA")
            .getchannel("A")
            .point(lambda a: 255 if a > 16 else 0)
            .getbbox()
        )
    if not box:
        raise ValueError(f"{path} has no visible pixels")
    return box


def _label_connected_8(mask: "np.ndarray"):
    """8-connected components of a boolean mask, as (labels, count)."""
    if _scipy_ndimage is not None:
        return _scipy_ndimage.label(mask, structure=np.ones((3, 3)))
    return _flood_labels(mask)


def _neighbours(cy, cx, h, w):
    """The 8 neighbours of one pixel that lie inside the image."""
    for ny in (cy - 1, cy, cy + 1):
        if not 0 <= ny < h:
            continue
        for nx in (cx - 1, cx, cx + 1):
            if 0 <= nx < w:
                yield ny, nx


def _flood_fill(mask, labels, start, label, h, w):
    """Label one component from `start`, four-way stack search over its 8-connected pixels."""
    labels[start] = label
    stack = [start]
    while stack:
        cy, cx = stack.pop()
        for ny, nx in _neighbours(cy, cx, h, w):
            if mask[ny, nx] and labels[ny, nx] == 0:
                labels[ny, nx] = label
                stack.append((ny, nx))


def _flood_labels(mask):
    """Fallback labeller for machines without scipy."""
    h, w = mask.shape
    labels = np.zeros((h, w), dtype=np.int32)
    current = 0
    ys, xs = np.nonzero(mask)
    for sy, sx in zip(ys.tolist(), xs.tolist()):
        if labels[sy, sx] != 0:
            continue
        current += 1
        _flood_fill(mask, labels, (sy, sx), current, h, w)
    return labels, current


def extract_component(src, index, dest):
    """Write only the index-th visible piece (ordered left to right) of a layer to its own PNG."""
    with Image.open(src) as image:
        rgba = np.array(image.convert("RGBA"))
    labels, count = _label_connected_8(rgba[:, :, 3] > 16)
    if count == 0:
        raise ValueError(
            f"{src} has 0 separate piece(s); component {index} does not exist"
        )
    areas = np.bincount(labels.ravel(), minlength=count + 1)
    # Stray specks are not pieces: only components of a meaningful size count (at least 2% of the largest).
    min_area = (
        min(50.0, float(areas[1:].max()))
        if areas[1:].max() < 50
        else max(50.0, 0.02 * float(areas[1:].max()))
    )
    pieces = [k for k in range(1, count + 1) if areas[k] >= min_area]
    if not 0 <= index < len(pieces):
        raise ValueError(
            f"{src} has {len(pieces)} separate piece(s); component {index} does not exist"
        )
    order = sorted(pieces, key=lambda k: np.where(labels == k)[1].min())
    rgba[labels != order[index], 3] = 0
    dest.parent.mkdir(parents=True, exist_ok=True)
    Image.fromarray(rgba).save(dest)
    return dest


def crop_layer_to_alpha(src, dest, pad=2):
    """Crop a full-canvas RGBA layer to its non-transparent bounding box (+pad px) to avoid
    rasterizing/uploading megabytes of empty transparent pixels on every motion-blur sample.
    Returns (cropped_path, [x0, y0]).
    """
    with Image.open(src) as image:
        rgba = image.convert("RGBA")
        w, h = rgba.size
        box = rgba.getchannel("A").point(lambda a: 255 if a > 0 else 0).getbbox()
        if not box:
            raise ValueError(f"{src} has no visible pixels")
        x0 = max(0, box[0] - pad)
        y0 = max(0, box[1] - pad)
        x1 = min(w, box[2] + pad)
        y1 = min(h, box[3] + pad)
        if x0 == 0 and y0 == 0 and x1 == w and y1 == h:
            return src, [0.0, 0.0]
        dest.parent.mkdir(parents=True, exist_ok=True)
        rgba.crop((x0, y0, x1, y1)).save(dest)
        return dest, [float(x0), float(y0)]


def resolve_pivot(pivot, image_path):
    if isinstance(pivot, list):
        return vector(pivot, 2, "pivot")
    if isinstance(pivot, dict) and pivot.get("of") == "alphaBounds":
        x0, y0, x1, y1 = alpha_bounds(image_path)
        cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
        table = {
            "top": (cx, y0),
            "bottom": (cx, y1),
            "left": (x0, cy),
            "right": (x1, cy),
            "center": (cx, cy),
            "topLeft": (x0, y0),
            "topRight": (x1, y0),
            "bottomLeft": (x0, y1),
            "bottomRight": (x1, y1),
        }
        if pivot.get("anchor") not in table:
            raise ValueError(f"pivot anchor must be one of {sorted(table)}")
        return list(table[pivot["anchor"]])
    raise ValueError('pivot must be [x, y] or {"anchor": ..., "of": "alphaBounds"}')


def _spec_canvas(spec):
    canvas = spec.get("canvas")
    if not (isinstance(canvas, list) and len(canvas) == 2):
        raise ValueError('spec needs "canvas": [width, height]')
    cw, ch = (
        int(number(canvas[0], "canvas width")),
        int(number(canvas[1], "canvas height")),
    )
    if cw <= 0 or ch <= 0:
        raise ValueError(f"canvas dimensions must be positive, got {[cw, ch]}")
    return cw, ch


def _spec_fps(spec):
    fps = number(spec.get("fps", 30), "fps")
    if fps <= 0:
        raise ValueError(f"fps must be positive, got {fps}")
    return fps


def _layer_source(base, item, lid, cw, ch):
    """The layer's image path, checked to exist and to match the canvas."""
    if "file" not in item or not isinstance(item["file"], str):
        raise ValueError(f'layer {lid}: needs a string "file" path')
    src = (base / item["file"]).resolve()
    if not src.is_file():
        raise ValueError(f"layer {lid}: image not found: {src}")
    with Image.open(src) as image:
        if list(image.size) != [cw, ch]:
            raise ValueError(
                f"layer {lid}: image is {image.size[0]}x{image.size[1]}, canvas is {cw}x{ch}"
            )
    return src


def _load_layer(item, lid, base, work, cw, ch, crop_layers):
    """One layer resolved: its image, optional cut-out component, pivot and crop origin."""
    src = _layer_source(base, item, lid, cw, ch)
    if "component" in item:
        src = extract_component(
            src, int(number(item["component"], "component")), work / f"{lid}.png"
        )
    pivot = resolve_pivot(item.get("pivot", [cw / 2, ch / 2]), src)
    crop_origin = [0.0, 0.0]
    if crop_layers:
        src, crop_origin = crop_layer_to_alpha(src, work / f"{lid}_crop.png")
    return {**item, "path": src, "pivot": pivot, "crop_origin": crop_origin}


def _check_layer_parents(layers):
    """Every parent must exist, and no chain may loop."""
    for lid, layer in layers.items():
        seen, cur = {lid}, layer.get("parent")
        while cur:
            if cur not in layers or cur in seen:
                raise ValueError(
                    f"layer {lid}: parent {cur!r} is unknown or forms a cycle"
                )
            seen.add(cur)
            cur = layers[cur].get("parent")


def _check_track(track, seen_tracks, layers):
    """One track: a unique id, a positive duration, and motion naming known layers and properties."""
    tid = track.get("id")
    if not isinstance(tid, str) or not tid or tid in seen_tracks:
        raise ValueError(f"track ids must be unique non-empty strings (got {tid!r})")
    seen_tracks.add(tid)
    dur = number(track.get("duration"), f"track {tid} duration")
    if dur <= 0:
        raise ValueError(f"track {tid}: duration must be positive, got {dur}")
    for mlid, mprops in (track.get("motion") or {}).items():
        if mlid not in layers:
            raise ValueError(f"track {tid}: motion references unknown layer {mlid!r}")
        if not isinstance(mprops, dict):
            raise ValueError(f"track {tid}, layer {mlid}: motion must be an object")
        for prop in mprops:
            if prop not in PROPS:
                raise ValueError(
                    f"track {tid}, layer {mlid}: unknown property {prop!r}; "
                    f"use one of {sorted(PROPS)}"
                )


def load_spec(path, work, crop_layers=True):
    spec = json.loads(Path(path).read_text())
    base = Path(path).resolve().parent
    cw, ch = _spec_canvas(spec)
    spec["canvas"] = [cw, ch]
    spec["fps"] = _spec_fps(spec)
    layers = {}
    for item in spec.get("layers", []):
        lid = item.get("id")
        if not isinstance(lid, str) or not lid or lid in layers:
            raise ValueError(
                f"layer ids must be unique non-empty strings (got {lid!r})"
            )
        layers[lid] = _load_layer(item, lid, base, work, cw, ch, crop_layers)
    if not layers:
        raise ValueError("spec needs at least one layer")
    _check_layer_parents(layers)
    tracks = spec.get("tracks")
    if not isinstance(tracks, list) or not tracks:
        raise ValueError("spec needs at least one track")
    seen_tracks = set()
    for track in tracks:
        _check_track(track, seen_tracks, layers)
    spec["layers"] = layers
    return spec


def keys_at(keys, t, size, name):
    if not isinstance(keys, list) or not keys:
        raise ValueError('"keys" needs at least one [time, value] pair')
    pairs = sorted(
        ([number(k[0], "key time"), vector(k[1], size, "key value")] for k in keys),
        key=lambda p: p[0],
    )
    if t <= pairs[0][0]:
        return pairs[0][1]
    for (t0, v0), (t1, v1) in zip(pairs, pairs[1:]):
        if t <= t1:
            u = ease(name, (t - t0) / (t1 - t0)) if t1 > t0 else 1.0
            return [a + (b - a) * u for a, b in zip(v0, v1)]
    return pairs[-1][1]


def envelope_at(term, t):
    env = term.get("envelope")
    if not env:
        return 1.0
    pts = sorted(
        (
            [number(pt[0], "envelope time"), number(pt[1], "envelope value")]
            for pt in env
        ),
        key=lambda p: p[0],
    )
    if t <= pts[0][0]:
        return pts[0][1]
    for (t0, v0), (t1, v1) in zip(pts, pts[1:]):
        if t <= t1:
            u = (t - t0) / (t1 - t0) if t1 > t0 else 1.0
            return v0 + (v1 - v0) * u
    return pts[-1][1]


def sampled_term(term, t, size):
    """Value of one non-oscillating term at time t."""
    kind = term.get("kind")
    if kind == "keys":
        return keys_at(term.get("keys"), t, size, term.get("ease", "smooth"))
    if kind == "ellipse":
        if size != 2:
            raise ValueError('"ellipse" applies to position only')
        t0, t1 = number(term.get("from", 0), "from"), number(term.get("to"), "to")
        u = ease(term.get("ease", "smooth"), (t - t0) / (t1 - t0)) if t1 > t0 else 1.0
        start = number(term.get("start", 0), "start")
        turns = number(term.get("turns", 1), "turns")
        a = start + 2 * math.pi * turns * u
        cx, cy = vector(term.get("centre"), 2, "centre")
        rx, ry = number(term.get("rx"), "rx"), number(term.get("ry"), "ry")
        default_origin = [cx + rx * math.cos(start), cy + ry * math.sin(start)]
        origin = vector(term.get("relativeTo", default_origin), 2, "relativeTo")
        return [cx + rx * math.cos(a) - origin[0], cy + ry * math.sin(a) - origin[1]]
    raise ValueError(f"Unknown motion term kind {kind!r}")


def oscillate_at(term, t, size):
    """Value of one oscillate term at time t (used when computing follow velocity)."""
    axis = int(number(term.get("axis", 0), "axis"))
    if not 0 <= axis < size:
        raise ValueError(f"oscillate axis {axis} is out of range for this property")
    amp = number(term.get("amplitude"), "amplitude")
    hz = number(term.get("hz"), "hz")
    ph = number(term.get("phase", 0), "phase")
    val = amp * math.sin(2 * math.pi * hz * t + ph) * envelope_at(term, t)
    out = [0.0] * size
    out[axis] = val
    return out


def js_envelope(term):
    env = term.get("envelope")
    if not env:
        return "1"
    pts = sorted(
        ([number(t, "envelope time"), number(v, "envelope value")] for t, v in env),
        key=lambda p: p[0],
    )
    pts_json = json.dumps(pts)
    return (
        "(function(t){var K=%s;if(t<=K[0][0])return K[0][1];for(var i=1;i<K.length;i++){"
        "if(t<=K[i][0]){var dt=K[i][0]-K[i-1][0];var u=dt>0?(t-K[i-1][0])/dt:1;"
        "return K[i-1][1]+(K[i][1]-K[i-1][1])*u;}}"
        "return K[K.length-1][1];})(time)"
    ) % pts_json


def oscillation_js(terms, size):
    parts = [[] for _ in range(size)]
    for term in terms:
        axis = int(number(term.get("axis", 0), "axis"))
        if not 0 <= axis < size:
            raise ValueError(f"oscillate axis {axis} is out of range for this property")
        amp, hz, ph = (
            number(term.get("amplitude"), "amplitude"),
            number(term.get("hz"), "hz"),
            number(term.get("phase", 0), "phase"),
        )
        parts[axis].append(
            f"{amp}*Math.sin(2*Math.PI*{hz}*time+{ph})*{js_envelope(term)}"
        )
    sums = ["(" + "+".join(p) + ")" if p else "0" for p in parts]
    return "value + " + (sums[0] if size == 1 else "[" + ",".join(sums) + "]")


def velocity_x(layer_terms, fps, duration):
    """Horizontal velocity (px/s) of a layer's position terms (keys, ellipse, and oscillate), frame by frame."""
    frames = max(1, int(round(duration * fps)) + 1)
    xs = []
    for i in range(frames):
        t = i / fps
        x = 0.0
        for term in layer_terms:
            kind = term.get("kind")
            if kind in ("keys", "ellipse"):
                x += sampled_term(term, t, 2)[0]
            elif kind == "oscillate":
                x += oscillate_at(term, t, 2)[0]
        xs.append(x)
    return [
        (xs[min(i + 1, frames - 1)] - xs[max(i - 1, 0)])
        * fps
        / (min(i + 1, frames - 1) - max(i - 1, 0) or 1)
        for i in range(frames)
    ]


KINDS = ("keys", "ellipse", "oscillate", "follow")


def check_terms(terms):
    """Every motion term must be an object of a known kind."""
    for term in terms:
        if not isinstance(term, dict) or term.get("kind") not in KINDS:
            raise ValueError(
                f"Unknown motion term kind "
                f"{term.get('kind') if isinstance(term, dict) else term!r}"
            )


def follow_targets(spec, track, follow, lid, fps, duration):
    """(term, horizontal velocity) for each follow term, refusing an unknown layer."""
    follows = []
    for term in follow:
        target_lid = term.get("layer", lid)
        if target_lid not in spec["layers"]:
            raise ValueError(f"follow references unknown layer {target_lid!r}")
        follows.append(
            (
                term,
                velocity_x(
                    track.get("motion", {}).get(target_lid, {}).get("position", []),
                    fps,
                    duration,
                ),
            )
        )
    return follows


def base_series(still, frames, fps, size):
    """The summed still terms (keys, ellipse) at every frame."""
    series = []
    for i in range(frames):
        value = [0.0] * size
        for term in still:
            value = [a + b for a, b in zip(value, sampled_term(term, i / fps, size))]
        series.append(value)
    return series


def apply_follow(follows, series, fps):
    """Add each follow term's smoothed lean to the rotation channel, in place."""
    for term, velocity in follows:
        smooth = max(0.0, number(term.get("smooth", 0.15), "smooth"))
        alpha = 1.0 if smooth == 0 else 1 - math.exp(-1.0 / (fps * smooth))
        lean, gain, limit = (
            0.0,
            number(term.get("gain"), "gain"),
            abs(number(term.get("limit"), "limit")),
        )
        for i, v in enumerate(velocity):
            lean += (max(-limit, min(limit, gain * v)) - lean) * alpha
            series[i][0] += lean


def channel(track, spec, lid, prop, fps, duration):
    """-> (keyframes [(t, value list)], expression or None) for one property of one layer."""
    size = PROPS[prop]
    terms = track.get("motion", {}).get(lid, {}).get(prop, [])
    check_terms(terms)
    still = [t for t in terms if t["kind"] not in ("oscillate", "follow")]
    osc = [t for t in terms if t["kind"] == "oscillate"]
    follow = [t for t in terms if t["kind"] == "follow"]
    if follow and prop != "rotation":
        raise ValueError('"follow" applies to rotation only')
    frames = max(1, int(round(duration * fps)) + 1)
    series = base_series(still, frames, fps, size)
    apply_follow(follow_targets(spec, track, follow, lid, fps, duration), series, fps)
    keyed = bool(still or follow)
    keys = [(i / fps, v) for i, v in enumerate(series)] if keyed else []
    return keys, (oscillation_js(osc, size) if osc else None)


def jsx_value(prop, offset, base_position):
    if prop == "position":
        return (
            f"[{base_position[0] + offset[0]:.4f},{base_position[1] + offset[1]:.4f},0]"
        )
    if prop == "scale":
        return f"[{100 + offset[0]:.4f},{100 + offset[1]:.4f},100]"
    if prop == "opacity":
        return f"{max(0.0, min(100.0, 100 + offset[0])):.4f}"
    return f"{offset[0]:.4f}"


def jsx_header():
    return [
        "var items = {};",
        "function get(id, path){ if(!items[id]) items[id] = app.project.importFile(new ImportOptions(new File(path))); return items[id]; }",
    ]


def jsx_comp(tid, cid, comp_w, comp_h, duration, fps, blur):
    """The composition for one track, with motion blur when the spec asks for it."""
    lines = [
        f"var comp_{cid} = app.project.items.addComp("
        f"{json.dumps(tid)}, {comp_w}, {comp_h}, 1.0, {duration:.17g}, {fps});"
    ]
    if blur:
        lines.append(
            f"comp_{cid}.motionBlur = true; comp_{cid}.shutterAngle = "
            f"{blur['shutterAngle']}; comp_{cid}.motionBlurSamplesPerFrame = "
            f"{blur['samples']};"
        )
    return lines


def layer_var(tid, lid):
    return re.sub(r"\W", "_", f"L_{tid}_{lid}")


def jsx_layer(lid, layer, cid, tid, shift, blur):
    """Import and place one layer inside its composition."""
    v = layer_var(tid, lid)
    sx, sy = shift
    crop_x0, crop_y0 = layer.get("crop_origin", [0.0, 0.0])
    anchor_x = layer["pivot"][0] - crop_x0
    anchor_y = layer["pivot"][1] - crop_y0
    lines = [
        f"var {v} = comp_{cid}.layers.add("
        f"get({json.dumps(lid)}, {json.dumps(str(layer['path']))}));",
        f"{v}.name = {json.dumps(lid)};",
        f'{v}.property("Anchor Point").setValue([{anchor_x:.4f},{anchor_y:.4f},0]);',
        f'{v}.property("Position").setValue([{layer["pivot"][0] + sx:.4f},{layer["pivot"][1] + sy:.4f},0]);',
    ]
    if blur and layer.get("motionBlur", True):
        lines.append(f"{v}.motionBlur = true;")
    return lines


def jsx_parents(spec, tid, names):
    """Parent links, all made before any keyframe exists: EffectCraft keeps a child's world
    position at the moment of linking, converting Position into the parent layer's local space."""
    lines = []
    for lid in names:
        layer = spec["layers"][lid]
        if layer.get("parent") and layer["parent"] in names:
            lines.append(
                f"{layer_var(tid, lid)}.parent = {layer_var(tid, layer['parent'])};"
            )
    return lines


def layer_origin(layer, spec, names, shift):
    """The resting position in composition space, or in the parent's local space when linked."""
    parent_id = layer.get("parent")
    if parent_id and parent_id in names:
        parent_crop = spec["layers"][parent_id].get("crop_origin", [0.0, 0.0])
        return [
            layer["pivot"][0] - parent_crop[0],
            layer["pivot"][1] - parent_crop[1],
        ]
    return [layer["pivot"][0] + shift[0], layer["pivot"][1] + shift[1]]


CHANNELS = (
    ("position", "Position"),
    ("rotation", "Rotation"),
    ("scale", "Scale"),
    ("opacity", "Opacity"),
)


def jsx_keys(track, spec, lid, v, origin, fps, duration):
    """Keyframes, hold interpolation and any expression for every property of one layer."""
    lines = []
    for prop, name in CHANNELS:
        keys, expression = channel(track, spec, lid, prop, fps, duration)
        handle = f"{v}.property({json.dumps(name)})"
        for t, value in keys:
            lines.append(
                f"{handle}.setValueAtTime({t:.17g}, {jsx_value(prop, value, origin)});"
            )
        terms = track.get("motion", {}).get(lid, {}).get(prop, [])
        if keys and terms and all(
            term.get("kind") == "keys" and term.get("ease") == "hold"
            for term in terms
        ):
            lines.append(
                f"for(var k=1;k<={handle}.numKeys;k++) {handle}.setInterpolationTypeAtKey("
                f"k,KeyframeInterpolationType.HOLD,KeyframeInterpolationType.HOLD);"
            )
        if expression:
            lines.append(f"{handle}.expression = {json.dumps(expression)};")
    return lines


def build_jsx(spec, project, work, blur):
    out = jsx_header()
    fps = spec["fps"]
    margin = vector(spec.get("margin", [0, 0, 0, 0]), 4, "margin")
    shift = margin[:2]
    comp_w, comp_h = (
        int(spec["canvas"][0] + margin[0] + margin[2]),
        int(spec["canvas"][1] + margin[1] + margin[3]),
    )
    for track in spec["tracks"]:
        tid, duration = track["id"], number(track["duration"], "duration")
        cid = re.sub(r"\W", "_", tid)
        names = track.get("layers") or list(spec["layers"])
        out += jsx_comp(tid, cid, comp_w, comp_h, duration, fps, blur)
        for lid in names:
            if lid not in spec["layers"]:
                raise ValueError(f"track {tid}: unknown layer {lid!r}")
            out += jsx_layer(lid, spec["layers"][lid], cid, tid, shift, blur)
        out += jsx_parents(spec, tid, names)
        for lid in names:
            layer = spec["layers"][lid]
            out += jsx_keys(
                track,
                spec,
                lid,
                layer_var(tid, lid),
                layer_origin(layer, spec, names, shift),
                fps,
                duration,
            )
    out.append(f"app.project.save(new File({json.dumps(str(project))}));")
    return "\n".join(out) + "\n"


def gpu_env(gpu: bool) -> dict[str, str]:
    """Return environment dict for effectcraft-cli; when gpu=True and the discrete NVIDIA Vulkan ICD
    is present, explicitly pin VK_DRIVER_FILES / VK_ICD_FILENAMES so Vulkan never falls back to an iGPU.
    """
    env = dict(os.environ)
    if gpu and NVIDIA_VULKAN_ICD.is_file():
        icd = str(NVIDIA_VULKAN_ICD)
        env.setdefault("VK_DRIVER_FILES", icd)
        env.setdefault("VK_ICD_FILENAMES", icd)
    return env


def opacity_series(track, lid, fps):
    """Opacity (0..100) of one layer at every frame of a track."""
    terms = track.get("motion", {}).get(lid, {}).get("opacity", [])
    out = []
    for i in range(max(1, int(round(track["duration"] * fps)))):
        t, value = i / fps, 100.0
        for term in terms:
            value += (
                oscillate_at(term, t, 1)
                if term["kind"] == "oscillate"
                else sampled_term(term, t, 1)
            )[0]
        out.append(max(0.0, min(100.0, value)))
    return out


def layer_masks(spec):
    """Each layer's opaque footprint on the canvas, at rest."""
    cw, ch = spec["canvas"]
    masks = {}
    for lid, layer in spec["layers"].items():
        with Image.open(layer["path"]) as image:
            alpha = np.array(image.convert("RGBA"))[:, :, 3] > 16
        x0, y0 = (
            int(round(layer["crop_origin"][0])),
            int(round(layer["crop_origin"][1])),
        )
        mask = np.zeros((ch, cw), bool)
        h, w = min(alpha.shape[0], ch - y0), min(alpha.shape[1], cw - x0)
        mask[y0 : y0 + h, x0 : x0 + w] = alpha[:h, :w]
        masks[lid] = mask
    return masks


def layer_ancestors(spec, lid):
    out, cur = set(), spec["layers"][lid].get("parent")
    while cur:
        out.add(cur)
        cur = spec["layers"][cur].get("parent")
    return out


def overlap_share(masks, a, b):
    """How much of the smaller footprint lies on the larger one, 0..1."""
    small = min(int(masks[a].sum()), int(masks[b].sum()))
    if not small:
        return 0.0
    return float(np.logical_and(masks[a], masks[b]).sum() / small)


def declared_alternates(declared, masks):
    """Validate the declared pairs and measure each one's overlap."""
    bad = any(
        not isinstance(pair, list)
        or len(pair) != 2
        or pair[0] == pair[1]
        or any(not isinstance(lid, str) or lid not in masks for lid in pair)
        for pair in declared
    )
    if not isinstance(declared, list) or bad:
        raise ValueError("alternates must be pairs of distinct, existing layer ids")
    return [(a, b, round(overlap_share(masks, a, b), 3)) for a, b in declared]


def inferred_alternates(spec, ids, masks):
    """Pairs that are neither ancestor of each other and mostly overlap at rest."""
    alternates = []
    for i, a in enumerate(ids):
        for b in ids[i + 1 :]:
            if a in layer_ancestors(spec, b) or b in layer_ancestors(spec, a):
                continue
            small, large = sorted((int(masks[a].sum()), int(masks[b].sum())))
            if not small or small / large < 0.15:
                continue
            shared = overlap_share(masks, a, b)
            if shared >= 0.6:
                alternates.append((a, b, round(shared, 3)))
    return alternates


def track_layer_issues(track, ids, spec, alternates):
    """Layers a track lists but never shows, and alternate pairs it shows at the same time."""
    issues = []
    names = track.get("layers") or ids
    opacity = {lid: opacity_series(track, lid, spec["fps"]) for lid in names}
    visible = {lid: [o > 50 for o in series] for lid, series in opacity.items()}
    for lid in names:
        if not any(o > 1 for o in opacity[lid]):
            issues.append(
                {
                    "level": "warning",
                    "track": track["id"],
                    "layer": lid,
                    "message": f"layer {lid} is listed but never visible in this track",
                }
            )
    for a, b, shared in alternates:
        if a not in visible or b not in visible:
            continue
        together = [
            f for f, (va, vb) in enumerate(zip(visible[a], visible[b])) if va and vb
        ]
        if not together:
            continue
        issues.append(
            {
                "level": "error",
                "track": track["id"],
                "layer": f"{a}+{b}",
                "message": f"{a} and {b} overlap ({shared:.0%} of the smaller) but are both "
                f"visible in {len(together)} frame(s), first at frame {together[0]}",
            }
        )
    return issues, set(names)


def unused_issues(ids, used):
    return [
        {
            "level": "info",
            "track": None,
            "layer": lid,
            "message": f"layer {lid} is not used by any track",
        }
        for lid in ids
        if lid not in used
    ]


def layer_checks(spec):
    """Checks the rendered clip cannot make: which layers are on screen together.

    Explicit alternate pairs take precedence over inference: two separate objects may intentionally overlap.
    Without a declaration, layers are inferred as alternates when neither is the other's ancestor,
    they are of similar scale, and most of the smaller one lies on the larger one at rest; showing two at once
    draws one over the other. Also reports layers a track lists but never shows, and layers no track uses."""
    masks = layer_masks(spec)
    ids = list(masks)
    declared = spec.get("alternates")
    if declared is not None:
        alternates = declared_alternates(declared, masks)
    else:
        alternates = inferred_alternates(spec, ids, masks)
    issues = []
    used = set()
    for track in spec["tracks"]:
        track_issues, names = track_layer_issues(track, ids, spec, alternates)
        issues += track_issues
        used.update(names)
    issues += unused_issues(ids, used)
    return {
        "alternates": [
            {"layers": [a, b], "overlap": shared} for a, b, shared in alternates
        ],
        "issues": issues,
    }


def check_options(resolution, quality):
    """Reject an option value before any work directory is touched."""
    if resolution not in RESOLUTIONS:
        raise ValueError(
            f"resolution must be one of {sorted(RESOLUTIONS)}, got {resolution!r}"
        )
    if quality not in QUALITIES:
        raise ValueError(f"quality must be one of {sorted(QUALITIES)}, got {quality!r}")


def restrict_tracks(spec, layers_report, only):
    """Keep the named tracks, and drop layer issues that belong to tracks left out."""
    unknown = sorted(set(only) - {t["id"] for t in spec["tracks"]})
    if unknown:
        raise ValueError(f"unknown track(s): {unknown}")
    spec["tracks"] = [t for t in spec["tracks"] if t["id"] in only]
    layers_report["issues"] = [
        i for i in layers_report["issues"] if i["track"] is None or i["track"] in only
    ]


def blur_settings(spec):
    """The motion-blur shutter and sample count, or None when the spec asks for none."""
    blur_spec = spec.get("motionBlur")
    if not blur_spec:
        return None
    return {
        "shutterAngle": number(blur_spec.get("shutterAngle", 360), "shutterAngle"),
        "samples": int(number(blur_spec.get("samples", 16), "samples")),
    }


def render_context(spec_path, out_dir, spec, *, gpu, resolution, quality, crf, stills, work):
    """Everything one render needs, resolved once."""
    return {
        "spec_path": spec_path,
        "out_dir": out_dir,
        "work": work,
        "spec": spec,
        "gpu": gpu,
        "resolution": resolution,
        "quality": quality,
        "crf": crf,
        "stills": stills,
        "project": work / "layered_motion.ecproj",
        "binary": shutil.which("effectcraft-cli") or "effectcraft-cli",
        "ffmpeg": shutil.which("ffmpeg") or "ffmpeg",
        "gpu_flag": ["--gpu"] if gpu else [],
        "env": gpu_env(gpu),
    }


def last_line(*streams):
    """The tool's most informative last line, for an error message."""
    for text in streams:
        lines = (text or "").strip().splitlines()
        if lines:
            return lines[-1]
    return "no output"


def run_script(ctx, script):
    """Build the layers into the EffectCraft project, or report why that failed."""
    done = subprocess.run(
        [ctx["binary"], "script", str(script)],
        capture_output=True,
        text=True,
        timeout=600,
        env=ctx["env"],
    )
    if done.returncode:
        raise RuntimeError(last_line(done.stderr, done.stdout))


def render_still(ctx, track, index, still):
    """One representative frame of a track, written transparent."""
    at = track["duration"] * (index + 0.5) / max(1, ctx["stills"])
    done = subprocess.run(
        [
            ctx["binary"],
            "--project",
            str(ctx["project"]),
            *ctx["gpu_flag"],
            "render-frame",
            "--comp",
            track["id"],
            "--time",
            f"{at:.4f}",
            "--transparent",
            "--out",
            str(still),
        ],
        capture_output=True,
        text=True,
        timeout=600,
        env=ctx["env"],
    )
    if done.returncode or not still.is_file():
        raise RuntimeError(
            f"Still {index} of track {track['id']} failed: "
            + last_line(done.stderr, done.stdout)
        )


def track_entry(ctx, track):
    """The reported shape of one track, rendering any requested stills first."""
    entry = {
        "track": track["id"],
        "fps": ctx["spec"]["fps"],
        "frames": int(round(track["duration"] * ctx["spec"]["fps"])),
    }
    if not ctx["stills"]:
        return entry
    folder = ctx["out_dir"] / "stills" / track["id"]
    folder.mkdir(parents=True, exist_ok=True)
    entry["stills"] = []
    for i in range(ctx["stills"]):
        still = folder / f"{i:02d}.png"
        render_still(ctx, track, i, still)
        entry["stills"].append(str(still))
    return entry


def render_frames(ctx, track, entry):
    """Render the track as numbered PNGs, refusing a partial frame set."""
    frames_dir = ctx["work"] / "frames" / track["id"]
    if frames_dir.exists():
        shutil.rmtree(frames_dir)
    frames_dir.mkdir(parents=True)
    done = subprocess.run(
        [
            ctx["binary"],
            "--project",
            str(ctx["project"]),
            *ctx["gpu_flag"],
            "render",
            "--comp",
            track["id"],
            "--out",
            str(frames_dir / "f.png"),
            "--format",
            "png",
            "--channels",
            "rgba",
            "--resolution",
            ctx["resolution"],
            "--quality",
            ctx["quality"],
        ],
        capture_output=True,
        text=True,
        timeout=3600,
        env=ctx["env"],
    )
    rendered = sorted(frames_dir.glob("f_*.png"))
    if done.returncode or len(rendered) != entry["frames"]:
        raise RuntimeError(
            f"Render of track {track['id']} failed "
            f"({len(rendered)}/{entry['frames']} frames): "
            + last_line(done.stderr, done.stdout)
        )
    return frames_dir


def encode_video(ctx, track, frames_dir, target):
    """Encode the rendered frames to VP9 with alpha, then drop the intermediate frames."""
    encoded = subprocess.run(
        [
            ctx["ffmpeg"],
            "-nostdin",
            "-v",
            "error",
            "-y",
            "-framerate",
            f"{ctx['spec']['fps']:g}",
            "-i",
            str(frames_dir / "f_%05d.png"),
            "-c:v",
            "libvpx-vp9",
            "-pix_fmt",
            "yuva420p",
            "-auto-alt-ref",
            "0",
            "-b:v",
            "0",
            "-crf",
            str(ctx["crf"]),
            "-row-mt",
            "1",
            "-threads",
            "4",
            "-deadline",
            "good",
            "-cpu-used",
            "2",
            str(target),
        ],
        capture_output=True,
        text=True,
        timeout=3600,
    )
    if encoded.returncode or not target.is_file() or target.stat().st_size == 0:
        raise RuntimeError(
            f"Encode of track {track['id']} failed: {last_line(encoded.stderr)}"
        )
    shutil.rmtree(frames_dir)


def review_reference(ctx, track):
    """The reference image this track is checked against, when the spec names one."""
    reference = ctx["spec"].get("reference")
    if not reference:
        return None
    return Path(ctx["spec_path"]).resolve().parent / reference


def analyze_track(ctx, track, entry, target, margin):
    """Validate the encoded clip and attach its measurements and review sheets."""
    ref_path = review_reference(ctx, track)
    report = analyze(
        target,
        expected_frames=entry["frames"],
        expected_fps=ctx["spec"]["fps"],
        loop=bool(track.get("loop")),
        reference=ref_path,
    )
    entry["size"] = [
        int(ctx["spec"]["canvas"][0] + margin[0] + margin[2]),
        int(ctx["spec"]["canvas"][1] + margin[1] + margin[3]),
    ]
    entry["review_sheets"] = [
        contact_sheet(
            target,
            ctx["out_dir"] / "review" / f"{track['id']}{suffix}.png",
            reference=ref_path,
            mode=mode,
        )["sheet"]
        for mode, suffix in (("path", ""), ("detail", "_detail"))
    ]
    return {
        **entry,
        "file": str(target),
        "validation": {
            "ok": report["ok"],
            "metrics": report["metrics"],
            "checks": report["checks"],
            "not_measured": report["not_measured"],
            "failed": [c for c in report["checks"] if not c["ok"]],
        },
    }


def validation_failures(outputs, layers_report, lint):
    """Tracks whose clip or layer checks failed, in report order."""
    problems = [
        o["track"] for o in outputs if "validation" in o and not o["validation"]["ok"]
    ]
    problems += sorted(
        {i["track"] for i in layers_report["issues"] if i["level"] == "error"}
        - set(problems)
    )
    return problems


def run(
    spec_path,
    out_dir,
    only=None,
    stills=0,
    video=True,
    gpu=True,
    crf=24,
    resolution="full",
    quality="best",
):
    check_options(resolution, quality)
    out_dir = Path(out_dir).resolve()
    work = out_dir / "work"
    work.mkdir(parents=True, exist_ok=True)
    spec = load_spec(spec_path, work)
    # Layer checks run on the whole spec, so "unused layer" is not judged from a subset.
    layers_report = layer_checks(spec)
    if only:
        restrict_tracks(spec, layers_report, only)
    blur = blur_settings(spec)
    ctx = render_context(
        spec_path,
        out_dir,
        spec,
        gpu=gpu,
        resolution=resolution,
        quality=quality,
        crf=crf,
        stills=stills,
        work=work,
    )
    script = work / "layered_motion.jsx"
    script.write_text(build_jsx(spec, ctx["project"], work, blur))
    run_script(ctx, script)
    outputs = []
    margin = vector(spec.get("margin", [0, 0, 0, 0]), 4, "margin")
    for track in spec["tracks"]:
        entry = track_entry(ctx, track)
        if not video:
            outputs.append(entry)
            continue
        target = out_dir / f"{track['id']}.webm"
        # Frames are rendered as PNGs (on the discrete GPU when asked) and encoded to VP9 with
        # alpha by ffmpeg, which is several times faster than EffectCraft's own WebM encode and
        # keeps the CPU cool.
        frames_dir = render_frames(ctx, track, entry)
        encode_video(ctx, track, frames_dir, target)
        entry["renderer"] = "gpu" if gpu else "cpu"
        outputs.append(analyze_track(ctx, track, entry, target, margin))
    lint = lint_spec(json.loads(Path(spec_path).read_text()))
    problems = validation_failures(outputs, layers_report, lint)
    return {
        "ok": True,
        "spec": str(Path(spec_path).resolve()),
        "project": str(ctx["project"]),
        "outputs": outputs,
        "spec_lint": lint,
        "layer_checks": layers_report,
        "validation_ok": not problems and not any(i["level"] == "error" for i in lint),
        "validation_failed_tracks": problems,
    }


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--spec", required=True)
    parser.add_argument("--out-dir", required=True)
    parser.add_argument(
        "--tracks", help="comma-separated track ids to build (default: all)"
    )
    parser.add_argument(
        "--stills",
        type=int,
        default=0,
        help="also render this many evenly spaced transparent stills per track",
    )
    parser.add_argument(
        "--cpu",
        action="store_true",
        help="render on the CPU instead of the GPU (default: GPU)",
    )
    parser.add_argument(
        "--gpu", action="store_true", help="explicitly render on the GPU (default)"
    )
    parser.add_argument(
        "--crf", type=int, default=24, help="VP9 quality (lower is better, default 24)"
    )
    parser.add_argument(
        "--resolution",
        default="full",
        choices=sorted(RESOLUTIONS),
        help="EffectCraft render resolution factor (default: full)",
    )
    parser.add_argument(
        "--quality",
        default="best",
        choices=sorted(QUALITIES),
        help="EffectCraft render quality (default: best)",
    )
    parser.add_argument(
        "--no-video",
        action="store_true",
        help="skip the WebM export (use with --stills for quick review)",
    )
    args = parser.parse_args()
    try:
        only = [t for t in (args.tracks or "").split(",") if t] or None
        use_gpu = False if args.cpu else True
        print(
            json.dumps(
                run(
                    args.spec,
                    args.out_dir,
                    only,
                    args.stills,
                    not args.no_video,
                    use_gpu,
                    args.crf,
                    args.resolution,
                    args.quality,
                )
            )
        )
    except Exception as exc:
        text = str(exc).strip().splitlines()
        parser.exit(1, f"{text[-1] if text else type(exc).__name__}\n")


if __name__ == "__main__":
    main()
