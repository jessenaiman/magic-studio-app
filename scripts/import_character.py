#!/usr/bin/env python3
"""Intelligent character import with automated Quality Gate (Option B) and interactive fallback (Option A).

Workflow:
  1. Runs PhotoCraft `select.subject` for automatic segmentation (zero hardcoded coordinates).
  2. Runs Magic Studio `vision.analyze` to verify coverage, alpha integrity, and fringe metrics.
  3. If Option B passes: automatically confirms and loads into Magic Studio.
  4. If Option B fails/warns: falls back to Option A (interactive prompt with metrics).
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


def run_cmd(args: list[str]) -> tuple[int, str, str]:
    res = subprocess.run(args, capture_output=True, text=True)  # noqa: S603 -- fixed argv
    return res.returncode, res.stdout, res.stderr


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path, help='Path to source character image')
    parser.add_argument('--name', type=str, help='Character asset name (default: file stem)')
    parser.add_argument('--out', type=Path, help='Custom output path for transparent PNG')
    parser.add_argument('--project', type=Path, help='Target .magic project for loading the asset')
    parser.add_argument('--no-load', action='store_true', help='Only process and verify; do not load into live app')
    parser.add_argument('--force', action='store_true', help='Bypass confirmation prompt if checks fail')
    args = parser.parse_args()
    if not args.no_load and args.project is None:
        parser.error('--project is required when loading; use --no-load to process only')
    return args


def resolve_inputs(args: argparse.Namespace) -> tuple[Path | None, Path, str, Path]:
    """Resolve the project, source, display name and the transparent output path."""
    project = args.project.resolve(strict=True) if args.project else None
    source = args.source.resolve(strict=True)
    name = args.name or source.stem.split('.')[0]
    # Clean name for asset ID
    asset_id = name.lower().replace(' ', '_').replace('-', '_')
    workspace_root = Path(__file__).resolve().parent.parent
    out_path = args.out or (
        workspace_root / 'exports' / 'pose_and_parts' / asset_id
        / f'{asset_id}_transparent.png'
    )
    out_path.parent.mkdir(parents=True, exist_ok=True)
    return project, source, name, out_path


def require_tools() -> tuple[str, str]:
    photocraft_bin = os.environ.get('PHOTOCRAFT_CLI') or shutil.which('photocraft-cli')
    animcraft_bin = shutil.which('magic-cli')
    if not photocraft_bin:
        sys.exit("Error: 'photocraft-cli' not found in PATH.")
    if not animcraft_bin:
        sys.exit("Error: 'magic-cli' not found in PATH.")
    return photocraft_bin, animcraft_bin


def report_bounds(stdout: str) -> None:
    for line in stdout.splitlines():
        if 'select.subject' in line:
            try:
                row = json.loads(line)
                bounds = row.get('result', {}).get('bounds')
                print(f"      Detected subject bounds: {bounds}")
            except Exception:  # noqa: S110 -- bounds are informational only
                pass


def segment(photocraft_bin: str, source: Path, out_path: Path) -> None:
    print(f"\n[1/3] Segmenting subject with photocraft-cli: {source.name}...")
    code, stdout, stderr = run_cmd([
        photocraft_bin, 'run', str(source),
        '--cmd', 'select.subject',
        '--cmd', 'layer.new.layerFromBackground', '--params', '{"ifPresent":true}',
        '--cmd', 'layer.layerMask.revealSelection',
        '--out', str(out_path),
    ])
    if code != 0 or not out_path.is_file():
        sys.exit(f"PhotoCraft segmentation failed: {stderr or stdout}")
    report_bounds(stdout)


def vision_metrics(animcraft_bin: str, out_path: Path) -> dict:
    print("\n[2/3] Evaluating Quality Gate (Option B) via vision.analyze...")
    code, stdout, stderr = run_cmd([animcraft_bin, 'exec', 'vision.analyze', f'image={out_path}'])
    if code != 0:
        sys.exit(f"Vision analysis failed: {stderr or stdout}")
    try:
        return json.loads(stdout).get('vision', {})
    except Exception as e:
        sys.exit(f"Failed to parse vision metrics: {e}")


def report_metrics(metrics: dict) -> None:
    print(f"      - Non-empty foreground : {metrics.get('non_empty', False)}")
    print(f"      - Foreground coverage  : {metrics.get('coverage_pct', 0.0):.2f}%")
    print(f"      - Opaque pixel count   : {metrics.get('opaque_px', 0):,}")
    print(f"      - Fringe edge ratio    : {metrics.get('fringe_ratio', 1.0):.4f}")
    print(f"      - Border clipped       : {metrics.get('border_clipped', False)}")


def gate_warnings(metrics: dict) -> list[str]:
    """Everything the automated Quality Gate considers evidence against the segmentation."""
    coverage = metrics.get('coverage_pct', 0.0)
    opaque_px = metrics.get('opaque_px', 0)
    fringe_ratio = metrics.get('fringe_ratio', 1.0)
    warnings = []
    if not metrics.get('non_empty', False):
        warnings.append("Subject appears completely empty (no foreground pixels).")
    if coverage < 2.0:
        warnings.append(f"Foreground coverage is very low ({coverage:.2f}% < 2.0%).")
    if coverage > 96.0:
        warnings.append(
            f"Foreground coverage is suspiciously high ({coverage:.2f}% > 96.0%), "
            "background may still be present."
        )
    if opaque_px < 500:
        warnings.append(f"Too few opaque pixels ({opaque_px} < 500).")
    if fringe_ratio > 0.09:
        warnings.append(f"High fringe edge ratio ({fringe_ratio:.4f} > 0.09).")
    return warnings


def operator_approval(args: argparse.Namespace, warnings: list[str], out_path: Path) -> bool:
    """Option B passes silently; otherwise fall back to Option A and ask."""
    if not warnings:
        print("\n  >>> [Option B: Automated Quality Gate PASSED] <<<")
        print("      Subject cleanly segmented, alpha confirmed intact.")
        return True
    print("\n  >>> [Option B: Quality Gate Flagged Warnings] <<<")
    for warning in warnings:
        print(f"      ! {warning}")
    if args.force:
        print("      --force flag specified. Proceeding despite warnings.")
        return True
    if not sys.stdin.isatty():
        print("      Non-interactive shell. Aborting without loading.")
        return False
    print("\n  >>> [Option A: Interactive Confirmation Fallback] <<<")
    print(f"      Review file: file://{out_path}")
    choice = input("      Load this asset into Magic Studio anyway? [y/N]: ").strip().lower()
    if choice in ('y', 'yes'):
        return True
    print("      Aborted by user.")
    sys.exit(0)


def load_asset(animcraft_bin: str, project: Path, out_path: Path, name: str) -> None:
    print(f"\n[3/3] Loading asset into Magic Studio project {project}...")
    code, stdout, stderr = run_cmd([
        animcraft_bin, '--project', str(project), 'exec', 'project.add_asset',
        f'path={out_path}', f'name={name}', 'kind=image',
    ])
    if code != 0:
        sys.exit(f"Failed to add asset to project: {stderr or stdout}")
    asset_id = json.loads(stdout)['asset_id']
    code, stdout, stderr = run_cmd([
        animcraft_bin, '--project', str(project), 'exec', 'project.load_asset',
        f'id={asset_id}', 'reload=true',
    ])
    if code == 0:
        print(f"      Successfully loaded '{name}' ({asset_id}) onto active Magic Studio stage!")
    else:
        print(f"      Warning: failed to load asset onto stage: {stderr or stdout}")


def main() -> None:
    args = parse_args()
    project, source, name, out_path = resolve_inputs(args)
    photocraft_bin, animcraft_bin = require_tools()
    segment(photocraft_bin, source, out_path)
    metrics = vision_metrics(animcraft_bin, out_path)
    report_metrics(metrics)
    if not operator_approval(args, gate_warnings(metrics), out_path):
        sys.exit(1)
    if args.no_load:
        print(f"\nDone. Output saved to: {out_path} (loading skipped).")
        return
    load_asset(animcraft_bin, project, out_path, name)


if __name__ == '__main__':
    main()
