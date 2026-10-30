"""Import media into FilmCraft and verify the resulting Program frame via its CLI."""

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess


def load_animation_into_filmcraft(
    media_path: str,
    project_out: str,
    frame_png_out: str,
    final_mp4_out: str | None = None,
    extra_assets: list[str] | None = None,
) -> dict:
    media = Path(media_path).resolve(strict=True)
    project, frame = Path(project_out).resolve(), Path(frame_png_out).resolve()
    project.parent.mkdir(parents=True, exist_ok=True)
    frame.parent.mkdir(parents=True, exist_ok=True)
    files = list(dict.fromkeys([str(media), *(str(Path(p).resolve(strict=True)) for p in extra_assets or [])]))
    binary = os.environ.get("FILMCRAFT_CLI") or shutil.which("filmcraft-cli")
    if not binary:
        raise RuntimeError("filmcraft-cli not found; set FILMCRAFT_CLI or put it on PATH")
    imported = subprocess.check_output(
        [binary, "--compact", "--save-as", str(project), "import", *files],
        text=True, timeout=30,
    )
    result = json.loads(imported)
    if result.get("errors") or not result.get("items"):
        raise RuntimeError(f"FilmCraft did not import the requested media: {result}")
    subprocess.run(
        [binary, "--project", str(project), "--save", "exec", "file.newSequenceFromClip",
         json.dumps({"items": result["items"]})],
        check=True, stdout=subprocess.DEVNULL, timeout=30,
    )
    inspected = subprocess.check_output(
        [binary, "--project", str(project), "inspect"], text=True, timeout=20,
    )
    probe = subprocess.check_output([binary, "probe", str(media)], text=True, timeout=20)
    subprocess.run(
        [binary, "--project", str(project), "render", "--seconds", "0", "--out", str(frame)],
        check=True, stdout=subprocess.DEVNULL, timeout=30,
    )
    if not project.is_file() or not frame.is_file() or not frame.stat().st_size:
        raise RuntimeError("FilmCraft did not write the project and verification frame")
    if final_mp4_out:
        destination = Path(final_mp4_out).resolve()
        destination.parent.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [binary, "--project", str(project), "export", str(destination)], check=True,
        )
    return {
        "importedToFilmCraft": True, "filmcraftProject": str(project), "projectExists": True,
        "programFramePng": str(frame), "frameExists": True, "mediaProbe": json.loads(probe),
        "importSummary": imported.strip(), "inspectExcerpt": inspected[:400],
    }


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("media")
    parser.add_argument("--project", required=True)
    parser.add_argument("--frame-out", required=True)
    args = parser.parse_args()
    print(json.dumps(load_animation_into_filmcraft(args.media, args.project, args.frame_out)))
