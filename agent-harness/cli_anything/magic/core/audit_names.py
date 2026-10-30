"""Reserved application names, and in-process copies of what those apps do.

reserved-names.json lists installed applications. Magic code calls those apps;
naming our files, functions, classes, modules or command IDs after them hides
whether the real app ran. ripgrep finds every occurrence; a name inside a native
file extension of that app (studio-template.deckcraft) is allowed.
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path

from cli_anything.magic.core.audit import executable

CONFIG = "reserved-names.json"
# Declarations in Rust, Python and JavaScript whose identifier contains a name.
DECLARATIONS = (
    r"^\s*(pub(\([a-z]+\))?\s+)?(async\s+)?(fn|struct|enum|mod|trait|const|static|type)\s+\w*({n})\w*",
    r"^\s*(async\s+)?(def|class|function)\s+\w*({n})\w*",
    r"^\s*(const|let|var)\s+\w*({n})\w*\s*=\s*(\(|async\b|function\b)",
)
# An ID with an app name in any segment: blender.start, part.open_photocraft.
COMMAND_ID = (
    r"\"[a-z_]+(?:\.[a-z_]+)*\.[a-z_]*({n})[a-z_]*(?:\.[a-z_]+)*\""  # later segment
    r"|\"[a-z_]*({n})[a-z_]*(?:\.[a-z_]+)+\""  # first segment
)


def config(repo: Path) -> dict:
    return json.loads((repo / CONFIG).read_text())


def rg(repo: Path, args: list[str]) -> list[str]:
    result = subprocess.run(  # noqa: S603 -- Fixed ripgrep argv; no shell.
        [executable("rg"), *args], cwd=repo, capture_output=True, text=True, check=False
    )
    return result.stdout.splitlines()


def scope(repo: Path, cfg: dict, within: Path | None) -> list[str]:
    """The configured code paths, or only PATH when a narrower target is audited."""
    if within is not None and within.resolve() != repo.resolve():
        return [str(within)]
    return [p for p in cfg["paths"] if (repo / p).exists()]


def files(repo: Path, cfg: dict, paths: list[str]) -> list[str]:
    names = [n.lower() for n in cfg["names"]]
    native = tuple(cfg["native_extensions"])
    found = []
    for path in rg(repo, ["--files", *paths]):
        stem = Path(path).name
        stem = stem[: -len(Path(path).suffix)] if path.endswith(native) else stem
        if any(name in stem.lower() for name in names):
            found.append(path)
    return found


def matches(
    repo: Path, cfg: dict, paths: list[str], patterns: list[str], *, only: bool = False
) -> list[str]:
    alternation = "|".join(cfg["names"])
    args = [
        "-n",
        "-i",
        "--no-heading",
        *(["-o", "--no-filename", "--no-line-number"] if only else []),
    ]
    for pattern in patterns:
        args += ["-e", pattern.replace("{n}", alternation)]
    return rg(repo, [*args, *paths])


def imitations(repo: Path, cfg: dict, paths: list[str]) -> dict[str, list[str]]:
    """Files importing a library that does an ArtCraft app's job, per app to call."""
    found = {}
    for app, libraries in cfg.get("imitations", {}).items():
        if app == "about":
            continue
        pattern = rf"^\s*(import|from)\s+({'|'.join(libraries)})\b"
        found[f"Files doing {app} work in-process"] = sorted(
            rg(repo, ["-l", "-e", pattern, *paths])
        )
    return found


def findings(repo: Path, cfg: dict, within: Path | None = None) -> dict[str, list[str]]:
    paths = scope(repo, cfg, within)
    return {
        **imitations(repo, cfg, paths),
        "Files named after apps": files(repo, cfg, paths),
        "Declarations named after apps": matches(repo, cfg, paths, list(DECLARATIONS)),
        "Command IDs named after apps": sorted(
            {
                found
                for line in matches(repo, cfg, paths, [COMMAND_ID], only=True)
                if not is_file_name(found := line.strip('"'), cfg)
            }
        ),
    }


FILE_SUFFIXES = {
    ".exe", ".js", ".json", ".py", ".rs", ".css", ".html", ".md", ".sh",
    ".png", ".jpg", ".webp", ".psd",
}


def is_file_name(text: str, cfg: dict) -> bool:
    """A quoted app file name (blender.exe, hyperframes.json) is not a command ID."""
    return text in cfg.get("native_files", []) or Path(text).suffix in FILE_SUFFIXES


def report(repo: Path, within: Path | None = None) -> int:
    """Print every finding, grouped; nonzero when any reserved name is in use."""
    found = findings(repo, config(repo), within)
    for title, items in found.items():
        print(f"{title}: {len(items)}")
        for item in items:
            print(f"  {item}")
    return 1 if any(found.values()) else 0
