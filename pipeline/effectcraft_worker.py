"""Freeze an editable EffectCraft document and render it in the shared native job."""

import hashlib
import json
from pathlib import Path
import shutil
import subprocess

from effectcraft_project import NativeProject, digest


def snapshot(project, job):
    source = project.path
    original_digest = digest(source)
    document = json.loads(source.read_text())
    items = document["items"]
    items = items.values() if isinstance(items, dict) else items
    bundle = job / "source-bundle"
    bundle.mkdir()
    dependencies, commands = [], []
    for item in items:
        kind = item.get("kind", {})
        if kind.get("type") != "Footage":
            continue
        original = (source.parent / kind["path"]).resolve()
        target = bundle / "media" / f"{item['id']}-{original.name}"
        target.parent.mkdir(exist_ok=True)
        shutil.copyfile(original, target)
        sha = digest(target)
        if sha != digest(original):
            raise ValueError("Native media changed while snapshotting")
        dependencies.append(dict(path=str(target.relative_to(bundle)), sha256=sha))
        commands.append(("footage.relink", dict(item=item["id"], path=str(target))))
    frozen = bundle / "source.ecproj"
    # Use the real document writer to relink. Never write the .ecproj schema ourselves.
    if not commands:
        shutil.copyfile(source, frozen)
    else:
        project.edit(commands, destination=frozen)
    if original_digest != digest(source):
        raise ValueError("Native document changed while snapshotting")
    dependencies.append(dict(path=frozen.name, sha256=digest(frozen)))
    return dict(path=str(frozen), bundle=str(bundle), original_path=str(source),
                sha256=digest(frozen), dependencies=dependencies,
                bundle_sha256=hashlib.sha256(json.dumps(dependencies, sort_keys=True).encode()).hexdigest())


def render(request, job):
    from worker import write_json
    from native_worker import collect_frames, encode_frames

    project = NativeProject(request["source"], request["executable"])
    frozen = snapshot(project, job)
    project.path = Path(frozen["path"])
    info = project.info()
    selection = request.get("comp", info["project"]["activeComp"])
    comps = [row for row in info["project"]["items"] if row["type"] == "Composition"]
    comp = next((row for row in comps if selection in (row["id"], row["name"])), None)
    if not comp:
        raise ValueError("Choose an existing EffectCraft composition")
    fps = comp["frameRate"]
    width, height = comp["size"]
    count = round(comp["duration"] * fps)
    first, last = request.get("first_frame", 0), request.get("last_frame", count - 1)
    if (type(first) is not int or type(last) is not int or not 0 <= first <= last < count
            or not 0 < fps <= 120 or not all(2 <= v <= 4096 for v in (width, height))
            or width * height * (last - first + 1) > 256 * 1024 * 1024):
        raise ValueError("Composition range exceeds native frame/rate/pixel limits")
    frames = job / "frames"
    frames.mkdir()
    argv = [project.executable, "--project", str(project.path), "render", "--comp", str(comp["id"]),
            "--out", str(frames / "f.png"), "--format", "png", "--channels", "rgba",
            "--start", format(first / fps, ".17g"), "--end", format((last + 1) / fps, ".17g"),
            "--audio", "off"]
    write_json(job / "source.json", frozen)
    write_json(job / "plan.json", dict(argv=argv, version=info["version"], composition=comp))
    write_json(job / "progress.json", dict(phase="Rendering EffectCraft", completed=0, total=last-first+1))
    subprocess.run(argv, check=True)
    measured = collect_frames(frames, last-first+1, width, height)
    encoded = encode_frames(request, job, [Path(f["path"]) for f in measured["frames"]], fps, width, height)
    return dict(encoded, backend="effectcraft", state="completed", source_bundle=frozen,
                tool=dict(path=project.executable, version=info["version"]), frames=str(frames),
                frame_mapping=measured["frames"], first_frame=first, last_frame=last, fps=fps,
                clip_seconds=(last-first+1)/fps, canvas=[width,height], world_rect=[0,0,width,height],
                partial=first != 0 or last != count-1, source_job=request["job"])
