"""Project management for Magic Studio (`.magic` files and HyperFrames studio websites)."""

from __future__ import annotations

import json
import os
from pathlib import Path



def _resolve_existing_file(p: Path) -> Path:
    if p.suffix.lower() == ".magic":
        return p.resolve()
    if p.name == "index.html" and p.parent.name == "hyperframes":
        found = list(p.parent.parent.glob("*.magic"))
        if len(found) == 1:
            return found[0].resolve()
    raise ValueError(f"{p} is not a .magic project file")


def _resolve_existing_dir(p: Path) -> Path:
    for anc in [p, *list(p.parents)[:2]]:
        found = sorted(anc.glob("*.magic"))
        if len(found) == 1:
            return found[0].resolve()
        if len(found) > 1:
            raise ValueError(f"{anc} has {len(found)} .magic files; specify the file path")
    raise ValueError(f"No .magic project file found in {p}")


def _lookup_recent_project(token: str) -> Path | None:
    config_home = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    recent_file = config_home / "magicstudio" / "recent_projects.json"
    if not recent_file.is_file():
        return None
    try:
        recent = json.loads(recent_file.read_text(encoding="utf-8"))
        low_token = token.lower()
        for entry in recent:
            ep = Path(entry)
            if ep.is_file() and low_token in (ep.stem.lower(), ep.name.lower()):
                return ep.resolve()
    except Exception:
        pass
    return None


def resolve_project_path(raw: str | Path) -> Path:
    """Resolve a `.magic` file path, a project directory, or a `hyperframes/` website folder."""
    p = Path(raw).expanduser()
    if p.is_file():
        return _resolve_existing_file(p)
    if p.is_dir():
        return _resolve_existing_dir(p)
    recent_match = _lookup_recent_project(str(raw).strip())
    if recent_match is not None:
        return recent_match
    if p.suffix.lower() != ".magic":
        p = p.with_suffix(".magic")
    return p.resolve() if p.exists() else p.absolute()
