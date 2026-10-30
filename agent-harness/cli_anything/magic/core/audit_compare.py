"""Before/now design and code metrics for one path, as a single table.

The same tools and the same current configuration measure the path twice:
as it was at a git revision and as it is in the working tree. Every metric
is lower-is-better, so each row reads better, worse or same at a glance.
"""

from __future__ import annotations

import json
import subprocess
import tempfile
from collections.abc import Callable
from pathlib import Path

from cli_anything.magic.core import audit, audit_concerns

CONFIGS = (
    "stylelint.config.mjs",
    ".jscpd.json",
    "purgecss.config.cjs",
    "code-limits.json",
    "eslint.config.mjs",
)


def tool(root: Path, name: str) -> str:
    """The repository's pinned node tool, else PATH."""
    local = root / "node_modules" / ".bin" / name
    return str(local) if local.exists() else name


def run(command: list[str], cwd: Path, stdin: bytes | None = None) -> str:
    return subprocess.run(
        command, cwd=cwd, input=stdin, capture_output=True, check=False
    ).stdout.decode()


def wallace(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    css = b"".join(p.read_bytes() + b"\n" for p in audit.css_sources(target))
    data = json.loads(run([tool(repo, "wallace"), "--json"], work, css))
    values, declarations = data["values"], data["declarations"]
    return {
        "CSS rules": data["rules"]["total"],
        "CSS declarations": declarations["total"],
        "Repeated declarations %": round(
            100 * (1 - declarations["uniquenessRatio"]), 1
        ),
        "ID selectors": data["selectors"]["id"]["total"],
        "!important": declarations["importants"]["total"],
        "Unique colours": values["colors"]["totalUnique"],
        "Unique font sizes": values["fontSizes"]["totalUnique"],
        "Unique z-indexes": values["zindexes"]["totalUnique"],
    }


def stylelint(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    report = work / "stylelint.json"
    run(
        [
            tool(repo, "stylelint"),
            "--config",
            str(repo / CONFIGS[0]),
            "--formatter",
            "json",
            "--output-file",
            str(report),
            str(target / "**/*.css"),
        ],
        work,
    )
    results = json.loads(report.read_text() or "[]")
    return {"Stylelint problems": sum(len(r["warnings"]) for r in results)}


def eslint(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    # The config's file patterns are repository-relative: run from the measured tree.
    command = [tool(repo, "eslint"), "--config", str(repo / CONFIGS[4])]
    results = json.loads(run([*command, "--format", "json", str(target)], tree))
    return {"Inline styles + JS lint (ESLint)": sum(len(r["messages"]) for r in results)}


def jscpd(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
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
        work,
    )
    total = json.loads((out / "jscpd-report.json").read_text())["statistics"]["total"]
    return {
        "Duplicated blocks (jscpd)": total["clones"],
        "Duplicated tokens %": round(total["percentageTokens"], 2),
    }


def purgecss(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    # The config's globs are repository-relative, so run from the measured tree.
    files = json.loads(
        run([tool(repo, "purgecss"), "-c", str(repo / CONFIGS[2])], tree)
    )
    return {"Dead selectors (PurgeCSS)": sum(len(f.get("rejected", [])) for f in files)}


def lizard(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    """File by file: one file Lizard cannot parse must not hide the rest."""
    options = [*audit.lizard_options(repo), "--warnings_only"]
    warnings = unparsed = 0
    for path in audit.sources(target, (".js", ".mjs", ".py", ".rs")):
        result = subprocess.run(
            [audit.executable("lizard"), str(path), *options],
            cwd=work,
            capture_output=True,
            text=True,
            check=False,
        )
        warnings += result.stdout.count(" warning: ")
        unparsed += "Traceback" in result.stderr or "Fail to parse" in result.stderr
    return {
        "Functions over limits (Lizard)": warnings,
        "Files Lizard cannot parse": unparsed,
    }


def concerns(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float]:
    return audit_concerns.metrics(repo, audit.css_sources(target))


MEASURES: tuple[Callable[[Path, Path, Path, Path], dict[str, float]], ...] = (
    stylelint,
    eslint,
    wallace,
    concerns,
    jscpd,
    purgecss,
    lizard,
)


def measure(repo: Path, tree: Path, target: Path, work: Path) -> dict[str, float | str]:
    """Every metric for target inside tree (the repository or a past checkout)."""
    metrics: dict[str, float | str] = {}
    for measure_one in MEASURES:
        try:
            metrics.update(measure_one(repo, tree, target, work))
        except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
            metrics[f"{measure_one.__name__} (not measured)"] = str(error)[:40]
    return metrics


def checkout(repo: Path, relative: Path, base: str, into: Path) -> Path:
    """The path as it was at base, in a scratch tree beside nothing else."""
    archive = subprocess.run(
        [audit.executable("git"), "archive", base, "--", str(relative)],
        cwd=repo,
        capture_output=True,
        check=True,
    ).stdout
    tar = [audit.executable("tar"), "-x", "-C", str(into)]
    subprocess.run(tar, input=archive, check=True)
    return into / relative


def scope(repo: Path, relative: Path, base: str) -> list[str]:
    """Git's numstat since base, split into inside and outside the path."""
    git = audit.executable("git")
    numstat = run([git, "diff", "--numstat", base, "--"], repo).splitlines()
    untracked = run([git, "ls-files", "--others", "--exclude-standard"], repo).split()
    inside, outside = [], []
    for line in numstat:
        added, removed, path = line.split("\t", 2)
        (inside if Path(path).is_relative_to(relative) else outside).append(
            f"{path} +{added} -{removed}"
        )
    for path in untracked:
        (inside if Path(path).is_relative_to(relative) else outside).append(
            f"{path} (new)"
        )
    return [
        f"Scope since {base}: {len(inside)} file(s) in {relative}, "
        f"{len(outside)} outside",
        *(f"  outside: {item}" for item in outside),
    ]


def verdict(before: float | str, now: float | str) -> str:
    if not isinstance(before, (int, float)) or not isinstance(now, (int, float)):
        return "?"
    return "better" if now < before else "worse" if now > before else "same"


def table(before: dict, now: dict) -> list[str]:
    rows = [
        (k, before.get(k, "-"), now.get(k, "-")) for k in dict.fromkeys([*before, *now])
    ]
    width = max(len(k) for k, _, _ in rows)
    lines = [f"{'Metric':<{width}}  {'Before':>8}  {'Now':>8}  {'Change':>8}  Verdict"]
    for key, old, new in rows:
        change = (
            f"{new - old:+g}"
            if isinstance(old, (int, float)) and isinstance(new, (int, float))
            else ""
        )
        lines.append(
            f"{key:<{width}}  {old!s:>8}  {new!s:>8}  {change:>8}  {verdict(old, new)}"
        )
    return lines


def compare(target: Path, base: str) -> int:
    """Print the before/now table; nonzero when any metric got worse."""
    repo = Path(
        run(
            ["git", "rev-parse", "--show-toplevel"],
            target if target.is_dir() else target.parent,
        ).strip()
    )
    relative = target.relative_to(repo)
    missing = [name for name in CONFIGS if not (repo / name).is_file()]
    if missing:
        print(
            f"audit --compare: missing repository configuration: {', '.join(missing)}"
        )
        return 2
    with tempfile.TemporaryDirectory(prefix="magic-audit-") as scratch:
        scratch_path = Path(scratch)
        before_root = scratch_path / "before"
        before_root.mkdir()
        (before_root / ".git").write_text("gitdir: unused\n")
        before_target = checkout(repo, relative, base, before_root)
        before = measure(repo, before_root, before_target, before_root)
        now_work = scratch_path / "now"
        now_work.mkdir()
        now = measure(repo, repo, target, now_work)
    sheets = audit.css_sources(target)
    print(
        "\n".join(
            [
                *table(before, now),
                "",
                *scope(repo, relative, base),
                "",
                *audit_concerns.report(repo, sheets),
            ]
        )
    )
    return 1 if any(verdict(before.get(k), v) == "worse" for k, v in now.items()) else 0
