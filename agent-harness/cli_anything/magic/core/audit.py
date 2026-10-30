"""Native repository checks, reported with unmodified tool output.

Each entry in TOOLS is a command that already exists, with the repository
configuration it expects. Everything below only decides what to run and where;
nothing here parses, scores, ranks or summarizes what the tool prints.
"""

from __future__ import annotations

import json
import shlex
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class ToolSpec:
    """A native command and the repository configuration it requires."""

    label: str
    command: tuple[str, ...]
    configs: tuple[str, ...]


TOOLS = {
    "ruff": ToolSpec(
        "Ruff",
        ("ruff", "check", "--no-cache", "--no-fix", "."),
        ("ruff.toml", ".ruff.toml"),
    ),
    "clippy": ToolSpec(
        "cargo clippy",
        (
            "cargo",
            "clippy",
            "--workspace",
            "--lib",
            "--bins",
            "--locked",
            "--offline",
            "--",
            "-D",
            "warnings",
        ),
        ("Cargo.toml",),
    ),
    "eslint": ToolSpec(
        "ESLint",
        ("eslint", "."),
        ("eslint.config.js", "eslint.config.mjs", "eslint.config.cjs"),
    ),
    "stylelint": ToolSpec(
        "Stylelint",
        ("stylelint", "**/*.css"),
        ("stylelint.config.*", ".stylelintrc*"),
    ),
    "lizard": ToolSpec("Lizard", ("lizard", "."), ("code-limits.json",)),
    "wallace": ToolSpec("Wallace", ("wallace",), ()),
    "jscpd": ToolSpec("jscpd", ("jscpd", "."), (".jscpd.json",)),
    "purgecss": ToolSpec(
        "PurgeCSS", ("purgecss", "-c", "purgecss.config.cjs"), ("purgecss.config.cjs",)
    ),
    "scope": ToolSpec("git scope", ("git", "diff", "--stat=160"), ()),
}

# Third-party stylesheets are measured by their own projects, not ours.
FOREIGN_CSS = {"vendor", "node_modules", "target"}

# A build emits `.js` beside the `.ts` it came from; measure the source.
GENERATED_UI = Path("apps/src-tauri/ui")


def generated_script_excludes(root: Path) -> list[str]:
    """Skip `.js` a build emits beside its `.ts`, so only the source is measured.

    Lizard matches `-x` against the paths it walks, which are the absolute target
    it was given; a repository-relative pattern silently matches nothing.
    """
    directory = root / GENERATED_UI
    if not directory.is_dir() or not any(directory.glob("*.ts")):
        return []
    return ["-x", str(directory / "*.js")]


def lizard_options(root: Path) -> list[str]:
    """The thresholds the repository already declares in code-limits.json."""
    limits = json.loads((root / "code-limits.json").read_text())
    if not isinstance(limits, dict):
        raise ValueError("code-limits.json must be an object")
    fields = (
        ("max_function_ccn", None, "cyclomatic_complexity"),
        ("max_function_lines", 100, "length"),
        ("max_function_cognitive_complexity", 15, "cognitive_complexity"),
    )
    options = ["-Ecognitive", "--sort", "cyclomatic_complexity"]
    for key, default, native_field in fields:
        limit = limits.get(key, default)
        if type(limit) is not int or limit < 1:
            raise ValueError(f"code-limits.json needs a positive integer {key}")
        options.extend(["--Threshold", f"{native_field}={limit}"])
    return options


def executable(name: str) -> str:
    """Absolute path of a system tool, so no command depends on the caller's cwd."""
    found = shutil.which(name)
    if found is None:
        raise FileNotFoundError(f"{name} not on PATH")
    return found


def has_config(directory: Path, name: str) -> bool:
    configs = TOOLS[name].configs
    return any(p.is_file() for pattern in configs for p in directory.glob(pattern))


def config_root(root: Path, name: str) -> Path:
    """Nearest directory up to the repository top that holds the tool's config.

    Clippy stays on the given crate directory; a missing config reports there.
    A tool without configuration runs from the repository top.
    """
    if name == "clippy":
        return root
    for directory in (root, *root.parents):
        if has_config(directory, name):
            return directory
        if (directory / ".git").exists():
            return root if TOOLS[name].configs else directory
    return root


def sources(target: Path, suffixes: tuple[str, ...]) -> list[Path]:
    """Our files with suffixes under target, in stable order, without vendor code."""
    if target.is_file():
        return [target]
    return sorted(
        path
        for path in target.rglob("*")
        if path.suffix in suffixes
        and not FOREIGN_CSS.intersection(path.relative_to(target).parts)
    )


def css_sources(target: Path) -> list[Path]:
    """Our stylesheets under target, in stable order, without third-party CSS."""
    return sources(target, (".css",))


def with_target(command: list[str], root: Path, target: Path, name: str) -> list[str]:
    """Point the native command at target when it is not the config root."""
    if name == "wallace":
        return [*command, str(target)] if target.is_file() else command
    if name in ("scope", "purgecss"):
        return command
    if target.is_file() or name == "lizard":
        command[-1] = str(target)
    elif target != root and name != "clippy":
        command[-1] = str(target / "**/*.css") if name == "stylelint" else str(target)
    return command


def build_command(root: Path, target: Path, name: str) -> list[str]:
    tool = TOOLS[name]
    if target.is_file() and name == "clippy":
        raise ValueError(
            "Clippy checks crates, not individual files; "
            "use a crate directory or --tool lizard for file complexity"
        )
    if tool.configs and not has_config(root, name):
        raise ValueError(f"missing repository configuration: {', '.join(tool.configs)}")
    command = with_target(list(tool.command), root, target, name)
    if name == "lizard":
        modules = target.with_suffix("")
        if target.is_file() and target.suffix == ".rs" and modules.is_dir():
            command.append(str(modules))
        command.extend(lizard_options(root))
        command.extend(generated_script_excludes(root))
    found = shutil.which(command[0]) or shutil.which(
        str(root / "node_modules" / ".bin" / command[0])
    )
    if found is None:
        raise FileNotFoundError(
            f"{command[0]} not on PATH or in root node_modules/.bin"
        )
    command[0] = found
    return command


def run_native(
    label: str,
    command: list[str],
    root: Path,
    dry_run: bool,
    *,
    stdin_files: list[Path] | None = None,
) -> int:
    """Run the tool and let it write to this terminal, unchanged."""
    shown = shlex.join(command)
    if stdin_files is not None:
        shown = f"cat {shlex.join(map(str, stdin_files))} | {shown}"
    print(f"[{label}] {shown}", file=sys.stderr, flush=True)
    if dry_run:
        print(f"[{label}] not run (--dry-run)", file=sys.stderr, flush=True)
        return 0
    stdin = None
    if stdin_files is not None:
        stdin = b"".join(path.read_bytes() + b"\n" for path in stdin_files)
    code = subprocess.run(  # noqa: S603 -- Fixed native tool argv; no shell.
        command, cwd=root, check=False, input=stdin
    ).returncode
    code = code if code >= 0 else 128 - code
    print(f"[{label}] exit {code}", file=sys.stderr, flush=True)
    return code


def run_scope(command: list[str], root: Path, base: str, dry_run: bool) -> int:
    """Git's own summary of the change since base, plus files git does not track."""
    code = run_native("git scope", [*command, base, "--"], root, dry_run)
    untracked = [command[0], "ls-files", "--others", "--exclude-standard"]
    return run_native("git untracked", untracked, root, dry_run) or code


def run_tool(
    name: str,
    command: list[str],
    root: Path,
    target: Path,
    *,
    dry_run: bool,
    base: str,
) -> int:
    """One tool's native run. Wallace takes a list of stylesheets on stdin."""
    label = TOOLS[name].label
    if name == "scope":
        return run_scope(command, root, base, dry_run)
    if name == "wallace" and target.is_dir():
        found = css_sources(target)
        if not found:
            raise ValueError(f"no stylesheets under {target}")
        return run_native(label, command, root, dry_run, stdin_files=found)
    return run_native(label, command, root, dry_run)


def run_audit(
    target: Path, tools: tuple[str, ...], *, dry_run: bool = False, base: str = "HEAD"
) -> int:
    """Run selected tools, inheriting output; return the first nonzero status."""
    single_file = target.is_file()
    selected = tools or (("lizard",) if single_file else tuple(TOOLS))
    start = target.parent if single_file else target
    status = 0
    for name in dict.fromkeys(selected):
        tool = TOOLS[name]
        try:
            root = config_root(start, name)
            code = run_tool(
                name,
                build_command(root, target, name),
                root,
                target,
                dry_run=dry_run,
                base=base,
            )
        except (OSError, ValueError) as error:
            print(f"[{tool.label}] not run: {error}", file=sys.stderr, flush=True)
            code = 2
        if status == 0:
            status = code
    return status
