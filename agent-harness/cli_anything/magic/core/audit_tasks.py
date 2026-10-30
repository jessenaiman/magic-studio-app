"""Task-sized audit findings an agent can take without searching.

Each task is one fix: the file and line, what the native tool reported, and what
to do with which installed tool. Tasks are ranked so the costliest problems come
first: code doing an ArtCraft app's job, app-named declarations, duplicated
blocks, complex functions, then UI normalization (inline styles, dead styles,
style-rule problems). Every source is a native report; nothing is scored.
"""

from __future__ import annotations

import json
import re
import subprocess
import tempfile
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path

from cli_anything.magic.core import audit, audit_names
from cli_anything.magic.core.audit_compare import CONFIGS, tool


@dataclass
class Task:
    kind: str
    file: str
    line: int
    action: str
    evidence: str
    weight: float


def run(command: list[str], cwd: Path) -> str:
    return subprocess.run(  # noqa: S603 -- Fixed tool argv; no shell.
        command, cwd=cwd, capture_output=True, text=True, check=False
    ).stdout


def imitation_tasks(repo: Path, cfg: dict, paths: list[str]) -> list[Task]:
    tasks = []
    hints = cfg.get("imitation_hints", {})
    for app, libraries in cfg.get("imitations", {}).items():
        if app == "about":
            continue
        pattern = rf"^\s*(import|from)\s+({'|'.join(libraries)})\b"
        hits: dict[str, list[str]] = {}
        for line in audit_names.rg(repo, ["-n", "--no-heading", "-e", pattern, *paths]):
            file, number, text = line.split(":", 2)
            hits.setdefault(rel(repo, file), []).append(f"{number}: {text.strip()}")
        operations = cfg.get("imitation_operations", {}).get(app, [])
        for file, lines in hits.items():
            edits = editing_calls(repo, file, operations)
            if operations and not edits:
                continue  # reads pixels only (QA, measurement): not an imitation
            count = sum(edits.values())
            tasks.append(
                Task(
                    "imitation",
                    file,
                    int(lines[0].split(":")[0]),
                    "Replace in-process editing with "
                    + hints.get(app, "a new magic-cli command")
                    + ".",
                    f"{count} editing calls: "
                    + ", ".join(f"{k}×{v}" for k, v in edits.most_common(6)),
                    90 + min(60, count),
                )
            )
    return tasks


def editing_calls(repo: Path, file: str, operations: list[str]) -> Counter:
    """How often each pixel-editing call appears in file, by its matched text."""
    found: Counter = Counter()
    if not operations:
        return found
    args = ["-o", "--no-filename", "--no-line-number"]
    for pattern in operations:
        args += ["-e", pattern]
    for hit in audit_names.rg(repo, [*args, file]):
        found[hit.strip(".(")] += 1
    return found


def name_tasks(repo: Path, cfg: dict, paths: list[str]) -> list[Task]:
    found = audit_names.findings(repo, cfg, None if len(paths) > 1 else Path(paths[0]))
    tasks = [
        Task("app-name", file, 1, "Rename the file by what our code does.", file, 80)
        for file in found["Files named after apps"]
    ]
    for line in found["Declarations named after apps"]:
        file, number, text = line.split(":", 2)
        tasks.append(
            Task(
                "app-name",
                file,
                int(number),
                "Rename the declaration by what it does;"
                " the app stays a call or parameter.",
                text.strip(),
                80,
            )
        )
    return tasks


def duplicate_tasks(repo: Path, target: Path, work: Path) -> list[Task]:
    out = work / "jscpd"
    run(
        [
            tool(repo, "jscpd"),
            str(target),
            "--config",
            str(repo / CONFIGS[1]),
            "--reporters",
            "json",
            "--output",
            str(out),
            "--silent",
        ],
        repo,
    )
    report = out / "jscpd-report.json"
    if not report.is_file():
        return []
    tasks = []
    for clone in json.loads(report.read_text())["duplicates"]:
        a, b = clone["firstFile"], clone["secondFile"]
        a["name"], b["name"] = (rel(repo, str(target / f["name"])) for f in (a, b))
        tasks.append(
            Task(
                "duplicate",
                a["name"],
                a["start"],
                f"Extract the shared block into one helper/class used by both places "
                f"({b['name']}:{b['start']}-{b['end']}).",
                f"{clone['tokens']} tokens, {clone['lines']} lines repeated",
                min(70, clone["tokens"] / 4),
            )
        )
    return tasks


WARNING = re.compile(r"^(.+?):(\d+): warning: (\S+) has (.+)$")


def complexity_tasks(repo: Path, target: Path) -> list[Task]:
    options = [*audit.lizard_options(repo), "--warnings_only"]
    tasks = []
    for path in audit.sources(target, (".js", ".mjs", ".py", ".rs")):
        for line in run(["lizard", str(path), *options], repo).splitlines():
            if match := WARNING.match(line):
                file, number, name, measures = match.groups()
                ccn = re.search(r"(\d+) CCN", measures)
                tasks.append(
                    Task(
                        "complexity",
                        rel(repo, file),
                        int(number),
                        f"Split {name} into smaller named steps"
                        " under the repository limits.",
                        measures,
                        min(60, int(ccn.group(1)) if ccn else 20),
                    )
                )
    return tasks


def rel(repo: Path, path: str) -> str:
    """A repository-relative path, whichever form the native tool reported."""
    candidate = Path(path)
    return str(candidate.relative_to(repo)) if candidate.is_absolute() else path


def inline_style_tasks(repo: Path, target: Path) -> list[Task]:
    command = [tool(repo, "eslint"), "--config", str(repo / CONFIGS[4])]
    results = json.loads(run([*command, "--format", "json", str(target)], repo) or "[]")
    return [
        Task(
            "inline-style",
            rel(repo, r["filePath"]),
            r["messages"][0]["line"],
            "Move inline style writes into a class or data-look in CSS.",
            f"{len(r['messages'])} inline style writes",
            min(40, 3 * len(r["messages"])),
        )
        for r in results
        if r["messages"]
    ]


def style_rule_tasks(repo: Path, target: Path, work: Path) -> list[Task]:
    report = work / "stylelint.json"
    command = [tool(repo, "stylelint"), "--config", str(repo / CONFIGS[0])]
    run(
        [
            *command,
            "--formatter",
            "json",
            "--output-file",
            str(report),
            str(target / "**/*.css"),
        ],
        repo,
    )
    tasks = []
    for result in json.loads(report.read_text() or "[]") if report.is_file() else []:
        rules: dict[str, list[int]] = {}
        for warning in result["warnings"]:
            rules.setdefault(warning["rule"], []).append(warning["line"])
        tasks += [
            Task(
                "style-rule",
                rel(repo, result["source"]),
                lines[0],
                f"Fix {rule}: use shared classes and :root tokens.",
                f"{len(lines)} at lines {', '.join(map(str, lines[:8]))}",
                min(35, 2 * len(lines)),
            )
            for rule, lines in rules.items()
        ]
    return tasks


def dead_style_tasks(repo: Path) -> list[Task]:
    command = [tool(repo, "purgecss"), "-c", str(repo / CONFIGS[2])]
    tasks = []
    for result in json.loads(run(command, repo) or "[]"):
        dead = [s.strip() for s in result.get("rejected", []) if s.strip()]
        if dead:
            tasks.append(
                Task(
                    "dead-style",
                    rel(repo, result["file"]),
                    1,
                    "Delete selectors no component or page produces"
                    " (check dynamic names).",
                    ", ".join(dead[:6]) + (" …" if len(dead) > 6 else ""),
                    min(30, 2 * len(dead)),
                )
            )
    return tasks


def collect(target: Path) -> list[Task]:
    repo = Path(
        run(
            ["git", "rev-parse", "--show-toplevel"],
            target if target.is_dir() else target.parent,
        ).strip()
    )
    cfg = audit_names.config(repo)
    paths = audit_names.scope(repo, cfg, target)
    with tempfile.TemporaryDirectory(prefix="magic-audit-tasks-") as scratch:
        work = Path(scratch)
        tasks = [
            *imitation_tasks(repo, cfg, paths),
            *name_tasks(repo, cfg, paths),
            *duplicate_tasks(repo, target, work),
            *complexity_tasks(repo, target),
            *inline_style_tasks(repo, target),
            *style_rule_tasks(repo, target, work),
            *dead_style_tasks(repo),
        ]
    return sorted(tasks, key=lambda t: (-t.weight, t.file, t.line))


def report(target: Path, limit: int, as_json: bool) -> int:
    """Print the top tasks; nonzero when any task remains."""
    tasks = collect(target)
    shown = tasks[:limit] if limit else tasks
    if as_json:
        print(json.dumps([asdict(t) for t in shown], indent=1))
    else:
        for number, t in enumerate(shown, 1):
            print(
                f"{number}. [{t.kind}] {t.file}:{t.line} — {t.action}\n   {t.evidence}"
            )
        print(
            f"\n{len(shown)} of {len(tasks)} tasks shown"
            " (--limit 0 shows all; --format json for agents)"
        )
        print("Agents use magic-cli only; it calls the *craft CLIs internally.")
    return 1 if tasks else 0
