#!/usr/bin/env python3
"""Install this built Magic Studio checkout's GUI/CLI and desktop entry locally.

Runtime dependencies are installed from agent-harness into an application-owned environment.
Build with: cargo build -p magicstudio-shell
This installs Magic only; it never installs or configures other Craft apps.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import subprocess


def migrate_legacy(root: Path, prefix: Path) -> None:
    """Migrate only recognized Magic entry points; leave other applications intact."""
    cli = prefix / "bin/magic-cli"
    codex = Path.home() / ".codex/config.toml"
    if codex.is_file():
        text = codex.read_text()
        pattern = r'(?ms)^\[mcp_servers\.mc\]\n(?P<body>.*?)(?=^\[|\Z)'
        match = re.search(pattern, text)
        if match and re.search(r'^command\s*=\s*"[^"\n]*/(?:magic-studio/bin|target/(?:debug|release))/mc"', match['body'], re.M):
            if '[mcp_servers.magicstudio]' in text:
                raise RuntimeError('Both old and new Magic MCP entries exist in Codex; resolve that conflict explicitly')
            body = re.sub(r'^command\s*=.*$', f'command = "{cli}"', match['body'], count=1, flags=re.M)
            codex.write_text(text[:match.start()] + '[mcp_servers.magicstudio]\n' + body + text[match.end():])
            print(f'Migrated existing Magic MCP entry: {codex}')
    opencode = Path.home() / '.config/opencode/opencode.json'
    if opencode.is_file():
        text = opencode.read_text()
        document = json.loads(text)
        for name in ('magicstudio', 'mc', 'animationcraft'):
            entry = document.get('mcp', {}).get(name, {})
            command = entry.get('command', [])
            if isinstance(command, list) and len(command) >= 2 and command[1] == 'mcp' and command[0] in {
                str(prefix / 'bin/mc'), str(root / 'target/debug/mc'),
                str(prefix / 'share/magic-studio/bin/mc'),
            }:
                text = text.replace(json.dumps(command[0]), json.dumps(str(cli)))
                print(f'Migrated existing Magic MCP entry: {opencode} ({name})')
        opencode.write_text(text)
    # Remove only this installation's obsolete command and duplicate desktop entry.
    old_link = prefix / 'bin/mc'
    if old_link.is_symlink() and old_link.resolve() == (prefix / 'share/magic-studio/bin/mc').resolve():
        old_link.unlink()
    for old in (prefix / 'share/magic-studio/bin/mc', root / 'target/debug/mc', root / 'target/release/mc'):
        if old.is_file():
            old.unlink()
            print(f'Removed obsolete Magic executable: {old}')
    desktop = prefix / 'share/applications/ai.storyteller.animationcraft.desktop'
    if desktop.is_file() and 'Name=Magic Studio' in desktop.read_text():
        desktop.unlink()
        print(f'Removed duplicate Magic desktop entry: {desktop}')
    # An earlier editable Python install registered magic as the terminal CLI.
    # Remove those known generated scripts, never another application's command.
    for directory in dict.fromkeys(os.environ.get('PATH', '').split(':')):
        candidate = Path(directory) / 'magic'
        if candidate.is_file() and candidate.resolve() != (prefix / 'bin/magic').resolve():
            try:
                content = candidate.read_text()
            except (UnicodeError, OSError):
                continue
            if 'from cli_anything.magic.magic_cli import main' in content:
                candidate.unlink()
                print(f'Removed stale Python Magic entry point: {candidate}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', type=Path, default=Path.home() / '.local')
    parser.add_argument('--migrate-legacy', action='store_true', help='Migrate recognized existing Magic MCP configuration and retire old entry points')
    parser.add_argument('--python', type=Path, help='Existing Python environment containing the Magic package and dependencies')
    parser.add_argument('--profile', choices=['debug', 'release'], default='debug')
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    build = root / 'target' / args.profile
    shell = build / 'magicstudio-shell'
    python = args.python.expanduser().absolute() if args.python else args.prefix / 'share/magic-studio/.venv/bin/python'
    if not shell.is_file():
        parser.error('Build the native shell first')
    if not python.is_file():
        if args.python:
            parser.error('The explicitly selected Python environment does not exist')
        subprocess.run(['uv', 'venv', str(python.parent.parent)], check=True)
    # Ship the CLI as an ordinary installed package, never an editable source link.
    subprocess.run(["uv", "pip", "install", "--python", str(python),
                    str(root / "agent-harness")], check=True)
    # Only tracked runtime resources are included; never projects, Git objects or build caches.
    runtime = args.prefix / "share/magic-studio/runtime"
    runtime.mkdir(parents=True, exist_ok=True)
    prefixes = ("pipeline/", "lib/", "schemas/", "automation/", "crew/", "backlot/",
                "tools/", ".agents/skills/")
    root_files = {"studio_pipeline_cli.py", "config.yaml", "SOURCE-ATTRIBUTION.md", "LICENSE"}
    tracked = subprocess.check_output(["git", "ls-files", "-z"], cwd=root).decode().split("\0")
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
        shutil.copy2(source, target)
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=root, text=True).strip()
    dirty = subprocess.check_output(["git", "diff", "--name-only", "HEAD"], cwd=root, text=True).splitlines()
    (runtime / "installation.json").write_text(json.dumps({
        "source_revision": revision, "modified_source_files": dirty,
        "native_sha256": hashlib.sha256(shell.read_bytes()).hexdigest(),
    }, indent=2))
    destination = args.prefix / 'share/magic-studio/bin'
    destination.mkdir(parents=True, exist_ok=True)
    # Replace the directory entry, not the running executable's inode.
    staged = destination / 'magicstudio-shell.new'
    shutil.copy2(shell, staged)
    staged.replace(destination / 'magicstudio-shell')
    for name, module in [('magic', 'gui'), ('magic-cli', 'magic_cli')]:
        entry = destination / name
        staged = destination / (name + '.new')
        staged.write_text(f'#!{python}\nfrom cli_anything.magic.{module} import main\nmain()\n')
        staged.chmod(0o755)
        staged.replace(entry)
        for link in [args.prefix / 'bin' / name, build / name]:
            link.parent.mkdir(parents=True, exist_ok=True)
            if link.is_symlink():
                link.unlink()
            elif link.exists():
                raise RuntimeError(f'Refusing to replace an unrelated regular executable: {link}')
            link.symlink_to(entry)
    shell_link = args.prefix / 'bin/magicstudio-shell'
    if not shell_link.exists() and not shell_link.is_symlink():
        shell_link.symlink_to(destination / 'magicstudio-shell')
    desktop = args.prefix / 'share/applications/ai.storyteller.magicstudio.desktop'
    desktop.parent.mkdir(parents=True, exist_ok=True)
    icon = args.prefix / 'share/icons/hicolor/256x256/apps/ai.storyteller.magicstudio.png'
    icon.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / 'apps/magicstudio-shell/icons/icon.png', icon)
    template = (root / 'apps/magicstudio-shell/magic.desktop').read_text()
    desktop.write_text(template.replace('Name=Magic Studio', 'Name=Magic ✨').replace('Exec=magic %f', f'Exec="{args.prefix / "bin/magic"}" %f'))
    if args.migrate_legacy:
        migrate_legacy(root, args.prefix)
    print(f'Installed Magic GUI, CLI and desktop entry under {args.prefix}')


if __name__ == '__main__':
    main()
