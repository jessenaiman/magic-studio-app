"""Stylesheet concerns: where each rule belongs in a layered file layout.

A stylesheet that holds many unrelated concerns is a god object. Each rule is
placed in an ITCSS-style layer, from least to most specific: tokens.css,
animations.css, base.css, components/<block>.css, views/<view>.css and
utilities.css. A component block is the first class or ID name segment
(.panel-head and .panel-body are both `panel`), so one file owns one look.
"""

from __future__ import annotations

import json
import re
import subprocess
from collections import defaultdict
from pathlib import Path

from cli_anything.magic.core.audit import executable

SCRIPT = Path(__file__).with_name("css_rules.mjs")
VIEW = re.compile(r"#view-([a-z]+)")
NAME = re.compile(r"[.#]([a-zA-Z][\w-]*)")
UTILITY = re.compile(r"^\.[a-z]+$")
# Blocks with fewer rules than this are listed together for review.
MIN_BLOCK_RULES = 3


def rules(repo: Path, files: list[Path]) -> list[dict]:
    """PostCSS's rules for files, one dict per rule or @keyframes."""
    if not files:
        return []
    postcss = repo / "node_modules" / "postcss"
    output = subprocess.run(  # noqa: S603 -- Fixed tool argv; no shell.
        [executable("node"), str(SCRIPT), str(postcss), *map(str, files)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    return [json.loads(line) for line in output.splitlines()]


def concern(rule: dict) -> str:
    """The suggested file for one rule."""
    selector = rule["selector"].split(",")[0].strip()
    props = rule["props"]
    if selector.startswith("@keyframes"):
        return "animations.css"
    if selector == ":root" or (props and all(p.startswith("--") for p in props)):
        return "tokens.css"
    if view := VIEW.search(selector):
        return f"views/{view.group(1)}.css"
    name = NAME.search(selector)
    if name is None:
        return "base.css"
    if UTILITY.match(selector) and len(props) <= 2:
        return "utilities.css"
    return f"components/{name.group(1).split('-')[0]}.css"


def grouped(found: list[dict]) -> dict[str, dict[str, list[int]]]:
    """Stylesheet -> suggested file -> rule line numbers."""
    files: dict[str, dict[str, list[int]]] = defaultdict(lambda: defaultdict(list))
    for rule in found:
        files[rule["file"]][concern(rule)].append(rule["line"])
    return files


def spread(files: dict[str, dict[str, list[int]]]) -> dict[str, list[str]]:
    """Components whose rules are split over more than one stylesheet."""
    owners: dict[str, list[str]] = defaultdict(list)
    for sheet, groups in files.items():
        for name in groups:
            if name.startswith("components/"):
                owners[name].append(Path(sheet).name)
    return {name: sheets for name, sheets in owners.items() if len(sheets) > 1}


def metrics(repo: Path, sheets: list[Path]) -> dict[str, float]:
    files = grouped(rules(repo, sheets))
    if not files:
        return {}
    largest = max(files.values(), key=lambda groups: sum(map(len, groups.values())))
    return {
        "Rules in largest stylesheet": sum(map(len, largest.values())),
        "Concerns in largest stylesheet": len(largest),
        "Components split across files": len(spread(files)),
    }


def describe(name: str, lines: list[int]) -> str:
    return f"    {name:<28} {len(lines):>4} rules  lines {min(lines)}-{max(lines)}"


def report(repo: Path, sheets: list[Path]) -> list[str]:
    """Suggested files per stylesheet, largest first, plus split components."""
    files = grouped(rules(repo, sheets))
    layers = "tokens → animations → base → components → views → utilities"
    out = [f"Stylesheet concerns ({layers})"]
    for sheet, groups in sorted(
        files.items(), key=lambda f: -sum(map(len, f[1].values()))
    ):
        total = sum(map(len, groups.values()))
        flag = "  ← god object: split by concern" if len(groups) > 8 else ""
        name = Path(sheet).relative_to(repo)
        out.append(f"  {name}: {total} rules, {len(groups)} concerns{flag}")
        small = []
        for name, lines in sorted(groups.items(), key=lambda g: (-len(g[1]), g[0])):
            if name.startswith("components/") and len(lines) < MIN_BLOCK_RULES:
                small.append(f"{name[11:-4]}:{','.join(map(str, lines))}")
            else:
                out.append(describe(name, lines))
        if small:
            hint = f"under {MIN_BLOCK_RULES} rules: merge or move"
            out.append(f"    review ({hint}): {' '.join(small)}")
    for name, owners in sorted(spread(files).items()):
        out.append(f"  split: {name} has rules in {', '.join(owners)}")
    return out
