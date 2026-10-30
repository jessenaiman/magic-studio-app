#!/usr/bin/env python3
"""Ask AssetFurnace for a rigged 3D model of one picture.

The model and its skeleton are saved beside the character. Runs under AssetFurnace's
own Python (``<home>/.venv/bin/python``) so its studio client is importable; Magic
calls the installed app and copies none of it.
"""

from __future__ import annotations

import argparse
import json
import shutil
import struct
import sys
import time
from pathlib import Path

import numpy as np


def _glb_json(path: Path) -> dict:
    data = path.read_bytes()
    length, kind = struct.unpack_from("<I4s", data, 12)
    if kind != b"JSON":
        raise ValueError(f"{path} has no JSON chunk")
    return json.loads(data[20 : 20 + length])


def _local_matrix(node: dict) -> np.ndarray:
    if "matrix" in node:
        return np.array(node["matrix"], dtype=float).reshape(4, 4).T
    x, y, z, w = node.get("rotation", [0, 0, 0, 1])
    rotation = np.array(
        [
            [1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w)],
            [2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w)],
            [2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y)],
        ]
    )
    matrix = np.eye(4)
    matrix[:3, :3] = rotation * np.array(node.get("scale", [1, 1, 1]))
    matrix[:3, 3] = node.get("translation", [0, 0, 0])
    return matrix


def skeleton_from_glb(path: Path) -> dict:
    """Joint names, parents and rest positions of the first skin (glTF metres, Y up)."""
    doc = _glb_json(path)
    nodes = doc.get("nodes", [])
    skins = doc.get("skins", [])
    if not skins:
        raise ValueError(f"{path} has no skeleton")
    parent = {
        child: i for i, node in enumerate(nodes) for child in node.get("children", [])
    }
    world: dict[int, np.ndarray] = {}

    def world_of(i: int) -> np.ndarray:
        if i not in world:
            above = world_of(parent[i]) if i in parent else np.eye(4)
            world[i] = above @ _local_matrix(nodes[i])
        return world[i]

    joints = skins[0]["joints"]
    names = {i: nodes[i].get("name", f"joint_{i}") for i in joints}
    return {
        "format": "magicstudio.skeleton",
        "source": str(path),
        "units": "metres, Y up (glTF)",
        "joints": [
            {
                "name": names[i],
                "parent": names.get(parent.get(i)),
                "head": [round(float(v), 5) for v in world_of(i)[:3, 3]],
            }
            for i in joints
        ],
    }


def make_rigged_model(
    image: Path, assetfurnace: Path, engine: str, timeout: int
) -> Path:
    sys.path.insert(0, str(assetfurnace))
    from image_to_3dlab import mcp_server as furnace

    studio = furnace.Studio()
    run = furnace.create(
        studio,
        want="character",
        image_path=str(image),
        engine=engine,
        stop_after_step="rigged",
    )
    deadline = time.time() + timeout
    state = furnace.run_status(studio, run_id=run["run_id"])
    while state.get("status") == "running" and time.time() < deadline:
        state = furnace.run_status(studio, run_id=run["run_id"], wait_seconds=60)
    rigged = (state.get("made") or {}).get("rigged")
    if state.get("status") != "done" or not rigged:
        raise RuntimeError(
            f"AssetFurnace did not rig the model: {json.dumps(state)[:1500]}"
        )
    return assetfurnace / rigged


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image", type=Path)
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="folder for rigged.glb and skeleton.json",
    )
    parser.add_argument("--assetfurnace", type=Path, required=True)
    parser.add_argument("--engine", default="pixal3d")
    parser.add_argument(
        "--timeout", type=int, default=3600, help="seconds to wait for the run"
    )
    args = parser.parse_args()
    rigged = make_rigged_model(
        args.image.resolve(), args.assetfurnace.resolve(), args.engine, args.timeout
    )
    args.out.mkdir(parents=True, exist_ok=True)
    model = args.out / "rigged.glb"
    shutil.copyfile(rigged, model)
    skeleton = skeleton_from_glb(model) | {
        "assetfurnace_file": str(rigged),
        "engine": args.engine,
    }
    (args.out / "skeleton.json").write_text(json.dumps(skeleton, indent=2))
    print(
        json.dumps(
            {
                "ok": True,
                "model": str(model),
                "skeleton": str(args.out / "skeleton.json"),
                "joints": len(skeleton["joints"]),
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
