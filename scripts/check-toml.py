#!/usr/bin/env python3
"""Fail on duplicate keys or invalid TOML in every tracked .toml file.

Cargo reports duplicate keys only when it parses a manifest; this checks every
tracked TOML file (Cargo.toml, Trunk.toml, rustfmt.toml, ...) before a commit.
"""
import subprocess
import sys
import tomllib
from pathlib import Path

SKIP = {".git", "target", "node_modules", ".runtime", "vendor"}


def tracked() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "-z", "*.toml"],
        capture_output=True,
        check=True,
    ).stdout
    paths = []
    for raw in out.split(b"\0"):
        if not raw:
            continue
        path = Path(raw.decode())
        if not SKIP.intersection(path.parts):
            paths.append(path)
    return paths


def main() -> int:
    bad = 0
    paths = tracked()
    for path in paths:
        try:
            with path.open("rb") as handle:
                tomllib.load(handle)
        except (tomllib.TOMLDecodeError, OSError) as error:
            print(f"{path}: {error}", file=sys.stderr)
            bad += 1
    if bad:
        print(f"{bad} of {len(paths)} TOML file(s) failed", file=sys.stderr)
        return 1
    print(f"checked {len(paths)} TOML file(s)", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
