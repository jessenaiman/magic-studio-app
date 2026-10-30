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
}


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


def build_command(root: Path, target: Path, name: str) -> list[str]:
    tool = TOOLS[name]
    if target.is_file() and name == "clippy":
        raise ValueError(
            "Clippy checks crates, not individual files; "
            "use a crate directory or --tool lizard for file complexity"
        )
    if not any(p.is_file() for pattern in tool.configs for p in root.glob(pattern)):
        raise ValueError(
            f"missing repository configuration: {', '.join(tool.configs)}"
        )
    command = list(tool.command)
    if target.is_file() or name == "lizard":
        command[-1] = str(target)
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
    label: str, command: list[str], root: Path, dry_run: bool, *, diagnostic=False
) -> int:
    print(f"[{label}] {shlex.join(command)}", file=sys.stderr, flush=True)
    if dry_run:
        print(f"[{label}] not run (--dry-run)", file=sys.stderr, flush=True)
        return 0
    code = subprocess.run(
        command, cwd=root, check=False, stdout=sys.stderr if diagnostic else None
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
    root = target.parent if single_file else target
    if single_file or set(selected) == {"lizard"}:
        root = next((p for p in (root, *root.parents) if (p / ".git").exists()), root)
    status = 0
    for name in dict.fromkeys(selected):
        tool = TOOLS[name]
        try:
            command = build_command(root, target, name)
            if name == "lizard" and output_format == "csv":
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
