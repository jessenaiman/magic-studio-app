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
    "lizard": ToolSpec("Lizard", ("lizard", "--csv", "."), ("code-limits.json",)),
}


def run_audit(root: Path, tools: tuple[str, ...], *, dry_run: bool = False) -> int:
    """Run selected tools, inheriting output; return the first nonzero status."""
    status = 0
    for name in dict.fromkeys(tools or TOOLS):
        tool = TOOLS[name]
        try:
            if not any(
                p.is_file() for pattern in tool.configs for p in root.glob(pattern)
            ):
                raise ValueError(
                    f"missing repository configuration: {', '.join(tool.configs)}"
                )
            command = list(tool.command)
            if name == "lizard":
                limits = json.loads((root / "code-limits.json").read_text())
                limit = (
                    limits.get("max_function_ccn") if isinstance(limits, dict) else None
                )
                if type(limit) is not int or limit < 1:
                    raise ValueError(
                        "code-limits.json needs a positive integer max_function_ccn"
                    )
                command.extend(["--CCN", str(limit)])
            executable = shutil.which(command[0]) or shutil.which(
                str(root / "node_modules" / ".bin" / command[0])
            )
            if executable is None:
                raise FileNotFoundError(
                    f"{command[0]} not on PATH or in root node_modules/.bin"
                )
            command[0] = executable
            print(f"[{tool.label}] {shlex.join(command)}", file=sys.stderr, flush=True)
            if dry_run:
                print(
                    f"[{tool.label}] not run (--dry-run)", file=sys.stderr, flush=True
                )
                continue
            code = subprocess.run(command, cwd=root, check=False).returncode
            code = code if code >= 0 else 128 - code
            print(f"[{tool.label}] exit {code}", file=sys.stderr, flush=True)
        except (OSError, ValueError) as error:
            print(f"[{tool.label}] not run: {error}", file=sys.stderr, flush=True)
            code = 2
        if status == 0:
            status = code
    return status
