#!/usr/bin/env python3
"""Install this built Magic Studio checkout's GUI/CLI and desktop entry locally.

Runtime dependencies from agent-harness use an application-owned environment.
Build with: cargo build -p magicstudio-shell -p backstage
This installs Magic only; it never installs or configures other Craft apps.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


def require_executable(name: str, parser: argparse.ArgumentParser) -> str:
    found = shutil.which(name)
    if not found:
        parser.error(f"Required executable not found on PATH: {name}")
    return str(Path(found).resolve())


def install_environment(
    root: Path, args: argparse.Namespace, parser: argparse.ArgumentParser, uv: str
) -> Path:
    python = (
        args.python.expanduser().absolute()
        if args.python
        else args.prefix / "share/magic-studio/.venv/bin/python"
    )
    if not python.is_file():
        if args.python:
            parser.error("The explicitly selected Python environment does not exist")
        if not sys.executable:
            parser.error("Cannot locate the Python interpreter running this installer")
        subprocess.run(
            [
                uv,
                "venv",
                "--python",
                sys.executable,
                "--no-python-downloads",
                str(python.parent.parent),
            ],
            check=True,
        )
    # Ship the CLI as an ordinary installed package, never an editable source link.
    subprocess.run(
        [
            uv,
            "pip",
            "install",
            "--no-python-downloads",
            "--python",
            str(python),
            str(root / "agent-harness"),
        ],
        check=True,
    )
    return python


def seed_crew_file(source: Path, target: Path) -> None:
    """Publish a complete seed without replacing an existing user file."""
    if target.exists() or target.is_symlink():
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=target.parent, delete=False) as dst:
            temporary = Path(dst.name)
            with source.open("rb") as src:
                shutil.copyfileobj(src, dst)
        shutil.copystat(source, temporary)
        os.link(temporary, target)
    except FileExistsError:
        pass
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def install_resources(
    root: Path, args: argparse.Namespace, shell: Path, git: str
) -> None:
    """Copy tracked runtime resources; seed crew files without replacing user edits."""
    runtime = args.prefix / "share/magic-studio/runtime"
    runtime.mkdir(parents=True, exist_ok=True)
    prefixes = (
        "pipeline/",
        "lib/",
        "schemas/",
        "automation/",
        "crew/",
        "backlot/",
        "tools/",
        ".agents/skills/",
    )
    root_files = {
        "studio_pipeline_cli.py",
        "config.yaml",
        "SOURCE-ATTRIBUTION.md",
        "LICENSE",
    }
    tracked = (
        subprocess.check_output([git, "ls-files", "-z"], cwd=root).decode().split("\0")
    )
    for name in tracked:
        if not name or not (name.startswith(prefixes) or name in root_files):
            continue
        relative = Path(name)
        if "tests" in relative.parts or relative.name.startswith("test_"):
            continue
        source = root / name
        if not source.is_file() or source.is_symlink():
            continue
        target = runtime / name
        target.parent.mkdir(parents=True, exist_ok=True)
        if relative.parts[0] == "crew":
            seed_crew_file(source, target)
        else:
            shutil.copy2(source, target)
    revision = subprocess.check_output(
        [git, "rev-parse", "HEAD"], cwd=root, text=True
    ).strip()
    dirty = subprocess.check_output(
        [git, "diff", "--name-only", "HEAD"], cwd=root, text=True
    ).splitlines()
    (runtime / "installation.json").write_text(
        json.dumps(
            {
                "source_revision": revision,
                "modified_source_files": dirty,
                "native_sha256": hashlib.sha256(shell.read_bytes()).hexdigest(),
            },
            indent=2,
        )
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prefix", type=Path, default=Path.home() / ".local")
    parser.add_argument(
        "--python",
        type=Path,
        help="Existing Python environment containing the Magic package "
        "and dependencies",
    )
    parser.add_argument("--profile", choices=["debug", "release"], default="debug")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    build = root / "target" / args.profile
    shell = build / "magicstudio-shell"
    if not shell.is_file():
        parser.error("Build the native shell first")
    backstage = build / "backstage"
    if not backstage.is_file():
        parser.error("Build the Backstage companion first: cargo build -p backstage")
    uv = require_executable("uv", parser)
    git = require_executable("git", parser)
    python = install_environment(root, args, parser, uv)
    install_resources(root, args, shell, git)
    destination = args.prefix / "share/magic-studio/bin"
    destination.mkdir(parents=True, exist_ok=True)
    # Replace the directory entry, not the running executable's inode.
    staged = destination / "magicstudio-shell.new"
    shutil.copy2(shell, staged)
    staged.replace(destination / "magicstudio-shell")
    staged = destination / "backstage.new"
    shutil.copy2(backstage, staged)
    staged.replace(destination / "backstage")
    for name, module in [("magic", "gui"), ("magic-cli", "magic_cli")]:
        entry = destination / name
        staged = destination / (name + ".new")
        staged.write_text(
            f"#!{python}\nfrom cli_anything.magic.{module} import main\nmain()\n"
        )
        staged.chmod(0o755)
        staged.replace(entry)
        for link in [args.prefix / "bin" / name, build / name]:
            link.parent.mkdir(parents=True, exist_ok=True)
            if link.is_symlink():
                link.unlink()
            elif link.exists():
                raise RuntimeError(
                    f"Refusing to replace an unrelated regular executable: {link}"
                )
            link.symlink_to(entry)
    for binary_name in ["magicstudio-shell", "backstage"]:
        link = args.prefix / "bin" / binary_name
        link.parent.mkdir(parents=True, exist_ok=True)
        if link.is_symlink():
            link.unlink()
        elif link.exists():
            raise RuntimeError(
                f"Refusing to replace an unrelated regular executable: {link}"
            )
        link.symlink_to(destination / binary_name)
    desktop = args.prefix / "share/applications/ai.storyteller.magicstudio.desktop"
    desktop.parent.mkdir(parents=True, exist_ok=True)
    icon = (
        args.prefix / "share/icons/hicolor/256x256/apps/ai.storyteller.magicstudio.png"
    )
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / "apps/magicstudio-shell/icons/icon.png", icon)
    template = (root / "apps/magicstudio-shell/magic.desktop").read_text()
    desktop.write_text(
        template.replace("Name=Magic Studio", "Name=Magic ✨").replace(
            "Exec=magic %f", f'Exec="{args.prefix / "bin/magic"}" %f'
        )
    )
    print(f"Installed Magic GUI, CLI and desktop entry under {args.prefix}")


if __name__ == "__main__":
    main()
