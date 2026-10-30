"""Build reusable native compositions from project-owned artwork and motion data."""

import argparse
import json
from pathlib import Path
import shutil
import tempfile

try:
    from .effectcraft_project import NativeProject, digest, fields
    from .effectcraft_motion import animate
except ImportError:
    from effectcraft_project import NativeProject, digest, fields
    from effectcraft_motion import animate


class SceneBuilder:
    def __init__(self, spec, output, executable=None):
        self.base = Path(spec).resolve().parent
        self.spec = json.loads(Path(spec).read_text())
        fields(
            self.spec, "name canvas fps duration layers controllers controls", "scene"
        )
        self.output = Path(output).resolve()
        self.output.mkdir(parents=True, exist_ok=False)
        self.work = tempfile.TemporaryDirectory(prefix="building-", dir=self.output)
        self.project = NativeProject(Path(self.work.name) / "scene.ecproj", executable)
        self.layers, self.puppets, self.media = {}, {}, []

    def build(self):
        try:
            w, h = self.spec["canvas"]
            self.project.edit(
                [
                    (
                        "comp.new",
                        dict(
                            name=self.spec.get("name", "Animation"),
                            width=w,
                            height=h,
                            frameRate=self.spec.get("fps", 24),
                            duration=self.spec["duration"],
                        ),
                    )
                ],
                new=True,
            )
            self.project.command(
                "essential.setPrimary", comp=self.project.info()["activeComp"]["id"]
            )
            self.controllers()
            for layer in self.spec["layers"]:
                self.add_layer(layer)
            self.project.edit(
                (
                    "layer.setParent",
                    dict(
                        layers=[self.layers[layer["id"]]],
                        parent=self.layers[layer["parent"]],
                        compensate=True,
                    ),
                )
                for layer in self.spec["layers"]
                if layer.get("parent")
            )
            for layer in self.spec["layers"]:
                result = animate(self.project, self.layers[layer["id"]], layer)
                if result:
                    self.puppets[layer["id"]] = result
            self.template()
            source = self.output / "animation.ecproj"
            shutil.move(self.project.path, source)
            self.project.path = source
            result = dict(
                source=str(source),
                template=str(self.output / "animation.ectemplate"),
                layers=self.layers,
                puppets=self.puppets,
                media=self.media,
                native=self.project.inspect(),
                commands=self.project.receipts,
            )
            (self.output / "receipt.json").write_text(
                json.dumps(result, indent=2) + "\n"
            )
            return result
        finally:
            self.work.cleanup()

    def add_layer(self, layer):
        fields(
            layer,
            "id file parent anchor position scale rotation "
            "properties puppet sketch model",
            "layer",
        )
        name = layer["id"]
        if name in self.layers or name == "Controls":
            raise ValueError(f"Duplicate or reserved layer ID: {name}")
        original = (self.base / layer["file"]).resolve()
        media = self.output / "media" / f"{len(self.media):04}-{original.name}"
        media.parent.mkdir(exist_ok=True)
        shutil.copyfile(original, media)
        sha = digest(media)
        if sha != digest(original):
            raise ValueError("Artwork changed during import")
        self.media.append(dict(source=str(original), file=str(media), sha256=sha))
        if "model" in layer:
            result = self.project.command("layer.newModel", path=str(media), name=name)
            uid = result["layer"]
        else:
            result = self.project.edit(
                [
                    (
                        "file.import",
                        dict(paths=[str(media)], sequence=False, addToComp=True),
                    ),
                    ("layer.rename", dict(name=name)),
                ]
            )
            info = self.project.info()
            uid = next(
                row["id"] for row in info["activeComp"]["layers"] if row["name"] == name
            )
        self.layers[name] = uid
        commands = [
            ("prop.set", dict(layer=uid, path=f"transform/{key}", value=layer[key]))
            for key in ("anchor", "position", "scale", "rotation")
            if key in layer
        ]
        if "model" in layer:
            commands.extend(self.model(uid, layer["model"]))
        self.project.edit(commands)

    def model(self, uid, model):
        fields(model, "clip loop speed unitScale", "model")
        commands = []
        if "clip" in model:
            options = self.project.enum_options(uid, "geometryOptions/animation")
            if model["clip"] not in options:
                raise ValueError(
                    f"Unknown animation clip {model['clip']!r}; available: {options}"
                )
            commands.append(
                (
                    "prop.set",
                    dict(
                        layer=uid,
                        path="geometryOptions/animation",
                        value=options.index(model["clip"]),
                    ),
                )
            )
        for key, path in [
            ("loop", "loopAnimation"),
            ("speed", "animationSpeed"),
            ("unitScale", "unitScale"),
        ]:
            if key in model:
                commands.append(
                    (
                        "prop.set",
                        dict(
                            layer=uid, path=f"geometryOptions/{path}", value=model[key]
                        ),
                    )
                )
        return commands

    def controllers(self):
        controllers = self.spec.get("controllers", [])
        if not controllers:
            return
        layer = self.project.command("layer.newNull", name="Controls")["layer"]
        for control in controllers:
            fields(control, "name value", "slider control")
            effect = self.project.command(
                "effect.apply", effect="Slider Control", layers=[layer]
            )
            path = effect["paths"][0] + "/slider"
            self.project.edit(
                [
                    (
                        "prop.renameGroup",
                        dict(
                            layer=layer, prop=effect["effects"][0], name=control["name"]
                        ),
                    ),
                    ("prop.set", dict(layer=layer, path=path, value=control["value"])),
                    (
                        "essential.addProperty",
                        dict(layer=layer, path=path, name=control["name"]),
                    ),
                ]
            )

    def template(self):
        self.project.command(
            "essential.setPrimary", comp=self.project.info()["activeComp"]["id"]
        )
        for control in self.spec.get("controls", []):
            fields(control, "layer path name media", "template control")
            params = dict(layer=self.layers[control["layer"]], name=control["name"])
            command = (
                "essential.addMedia"
                if control.get("media")
                else "essential.addProperty"
            )
            if command == "essential.addProperty":
                params["path"] = control["path"]
            self.project.command(command, **params)
        self.project.command(
            "essential.exportTemplate",
            path=str(self.output / "animation.ectemplate"),
            name=self.spec.get("name", "Animation"),
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["build", "inspect"])
    parser.add_argument("source", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--tool")
    args = parser.parse_args()
    result = (
        SceneBuilder(args.source, args.output, args.tool).build()
        if args.operation == "build"
        else NativeProject(args.source, args.tool).inspect()
    )
    print(json.dumps(result))


if __name__ == "__main__":
    main()
