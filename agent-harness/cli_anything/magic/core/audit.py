"""Native repository audit planning and execution; no report translation."""

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
    # Project Wallace design metrics: unique values per property, selector
    # specificity and declaration reuse, in its own table or JSON.
    "wallace": ToolSpec("Wallace", ("wallace",), ()),
}

# Third-party stylesheets are measured by their own projects, not ours.
FOREIGN_CSS = {"vendor", "node_modules", "target"}


def lizard_options(root: Path) -> list[str]:
    """Use native metrics and thresholds, including long low-branch functions."""
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


def css_sources(target: Path) -> list[Path]:
    """Our stylesheets under target, in stable order, without third-party CSS."""
    if target.is_file():
        return [target]
    return sorted(
        path for path in target.rglob("*.css")
        if not FOREIGN_CSS.intersection(path.relative_to(target).parts)
    )


def with_target(command: list[str], root: Path, target: Path, name: str) -> list[str]:
    """Point the native command at target when it is not the config root."""
    if name == "wallace":
        return [*command, str(target)] if target.is_file() else command
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
        raise ValueError(
            f"missing repository configuration: {', '.join(tool.configs)}"
        )
    command = with_target(list(tool.command), root, target, name)
    if name == "lizard":
        modules = target.with_suffix("")
        if target.is_file() and target.suffix == ".rs" and modules.is_dir():
            command.append(str(modules))
        command.extend(lizard_options(root))
    executable = shutil.which(command[0]) or shutil.which(
        str(root / "node_modules" / ".bin" / command[0])
    )
    if executable is None:
        raise FileNotFoundError(
            f"{command[0]} not on PATH or in root node_modules/.bin"
        )
    command[0] = executable
    return command


def run_native(
    label: str, command: list[str], root: Path, dry_run: bool, *, diagnostic=False,
    stdin_files: list[Path] | None = None,
) -> int:
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
    code = subprocess.run(
        command, cwd=root, check=False, input=stdin,
        stdout=sys.stderr if diagnostic else None,
    ).returncode
    code = code if code >= 0 else 128 - code
    print(f"[{label}] exit {code}", file=sys.stderr, flush=True)
    return code


def run_audit(
    target: Path, tools: tuple[str, ...], *, dry_run: bool = False,
    output_format: str = "text",
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
            command = build_command(root, target, name)
            if name == "wallace" and not single_file:
                sources = css_sources(target)
                if not sources:
                    raise ValueError(f"no stylesheets under {target}")
                code = run_native(
                    tool.label, command, root, dry_run, stdin_files=sources
                )
            elif name == "lizard" and output_format == "csv":
                # Lizard's CSV printer returns zero even when thresholds fail.
                # Keep stdout native CSV; use a native warning pass on stderr
                # for enforcement, without inventing a report parser or score.
                code = run_native(tool.label, [*command, "--csv"], root, dry_run)
                warning_code = run_native(
                    "Lizard thresholds", [*command, "--warnings_only"],
                    root, dry_run, diagnostic=True,
                )
                code = code or warning_code
            else:
                code = run_native(tool.label, command, root, dry_run)
        except (OSError, ValueError) as error:
            print(f"[{tool.label}] not run: {error}", file=sys.stderr, flush=True)
            code = 2
        if status == 0:
            status = code
    return status
