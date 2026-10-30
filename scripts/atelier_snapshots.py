"""Render one review still per scene for an atelier (bespoke) composition.

The Backlot storyboard can't thumbnail a `.tsx` scene, so a bespoke run
populates the assets-gate filmstrip by writing `projects/<slug>/snapshots/
<scene_id>.png` — one Remotion `still` per scene at a representative frame.
Run this AT THE ASSETS GATE (before any draft/compose render):

    python scripts/atelier_snapshots.py <slug>

It reads scene timings from `artifacts/scene_plan.json` and the bespoke render
config from `artifacts/edit_decisions.json` (falling back to conventional
paths: index.tsx / artifacts/props.json / public/). The composition id comes
from edit_decisions.bespoke.composition_id or --composition-id.

See skills/meta/bespoke-composition.md and skills/meta/checkpoint-protocol.md.
"""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

# On Windows npx is npx.cmd — resolve it so subprocess finds it without a shell.
NPX = shutil.which("npx") or "npx"

REPO_ROOT = Path(__file__).resolve().parent.parent
COMPOSER_DIR = REPO_ROOT / "remotion-composer"


def _load(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def _scenes(proj: Path) -> list:
    plan = _load(proj / "artifacts" / "scene_plan.json")
    return (plan.get("scenes") or []) if isinstance(plan, dict) else []


def _bespoke(proj: Path) -> dict:
    edit = _load(proj / "artifacts" / "edit_decisions.json")
    return (edit.get("bespoke") or {}) if isinstance(edit, dict) else {}


def _render_paths(args: argparse.Namespace, proj: Path, bespoke: dict):
    """The staged inputs one still needs: props, entry, public dir and composition id."""
    props_path = Path(
        args.props or bespoke.get("props_path") or (proj / "artifacts" / "props.json")
    )
    entry = Path(args.entry or bespoke.get("entry") or (proj / "index.tsx"))
    if not entry.is_absolute():
        entry = (REPO_ROOT / entry).resolve()
    public_dir = Path(args.public_dir or bespoke.get("public_dir") or (proj / "public"))
    comp_id = args.composition_id or bespoke.get("composition_id")
    return props_path, entry, public_dir, comp_id


def _fps(args: argparse.Namespace, props_path: Path) -> int:
    if args.fps is not None:
        return args.fps
    return int(_load(props_path).get("fps") or 30)


def _scene_frame(scene: dict, only: list[str] | None, fps: int):
    """(scene id, representative frame) — or None when this scene is skipped."""
    sid = str(scene.get("id") or "").strip()
    if not sid or (only and sid not in only):
        return None
    start = scene.get("start_seconds")
    end = scene.get("end_seconds")
    mid = ((start + end) / 2) if (start is not None and end is not None) else (start or 0)
    return sid, max(0, round(mid * fps))


def _render_still(entry: Path, comp_id: str, out: Path, frame: int, props: Path, public: Path):
    """Render one still; returns the failure text, or None on success."""
    cmd = [
        NPX, "remotion", "still", str(entry), str(comp_id), str(out.resolve()),
        f"--frame={frame}",
        f"--props={props.resolve()}",
        f"--public-dir={public.resolve()}",
    ]
    try:
        subprocess.run(  # noqa: S603 -- fixed argv from the composition id
            cmd, cwd=COMPOSER_DIR, check=True, capture_output=True, text=True, timeout=600
        )
        return None
    except subprocess.CalledProcessError as e:
        return (e.stderr or e.stdout or "")[-300:]
    except Exception as e:  # noqa: BLE001
        return str(e)


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("slug", help="project slug under projects/")
    ap.add_argument("--composition-id", help="Remotion composition id (else from edit_decisions)")
    ap.add_argument("--entry", help="entry .tsx (default projects/<slug>/index.tsx)")
    ap.add_argument("--props", help="props JSON (default artifacts/props.json)")
    ap.add_argument("--public-dir", help="public dir (default projects/<slug>/public)")
    ap.add_argument("--fps", type=int, default=None, help="frames per second (default from props or 30)")
    ap.add_argument("--only", nargs="*", help="only these scene ids")
    args = ap.parse_args(argv)

    proj = REPO_ROOT / "projects" / args.slug
    if not proj.is_dir():
        print(f"error: no project at {proj}", file=sys.stderr)
        return 2

    scenes = _scenes(proj)
    if not scenes:
        print("error: no scenes in artifacts/scene_plan.json", file=sys.stderr)
        return 2

    props_path, entry, public_dir, comp_id = _render_paths(args, proj, _bespoke(proj))
    if not comp_id:
        print(
            "error: composition id unknown (pass --composition-id or set edit_decisions.bespoke)",
            file=sys.stderr,
        )
        return 2
    fps = _fps(args, props_path)

    # Stage the project into remotion-composer so webpack resolves node_modules.
    sys.path.insert(0, str(REPO_ROOT))
    from tools.video.video_compose import VideoCompose  # noqa: E402
    staged_entry = VideoCompose()._stage_atelier_project(entry, COMPOSER_DIR)

    snap_dir = proj / "snapshots"
    snap_dir.mkdir(exist_ok=True)

    ok, fail = 0, 0
    for scene in scenes:
        selected = _scene_frame(scene, args.only, fps)
        if selected is None:
            continue
        sid, frame = selected
        out = snap_dir / f"{sid}.png"
        failure = _render_still(staged_entry, comp_id, out, frame, props_path, public_dir)
        if failure is None:
            ok += 1
            print(f"  {sid}: frame {frame} -> {out.relative_to(REPO_ROOT)}")
        else:
            fail += 1
            print(f"  {sid}: FAILED — {failure}", file=sys.stderr)

    print(f"snapshots: {ok} ok, {fail} failed -> {snap_dir.relative_to(REPO_ROOT)}")
    return 0 if fail == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
