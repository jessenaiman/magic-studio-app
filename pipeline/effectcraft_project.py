"""Editable EffectCraft documents. All writes go through the installed native CLI."""

import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fields(value, allowed, label):
    if not isinstance(value, dict) or set(value) - set(allowed.split()):
        raise ValueError(f"Unsupported {label} fields; expected {allowed}")
    return value


def nodes(tree):
    if isinstance(tree, dict):
        yield tree
        for child in tree.values():
            yield from nodes(child)
    elif isinstance(tree, list):
        for child in tree:
            yield from nodes(child)


def effectcraft_cli():
    explicit = os.environ.get("EFFECTCRAFT_CLI")
    if explicit:
        return explicit
    home = Path.home() / ".local/share"
    try:
        installed = json.loads((home / "artcraft-launcher/installed.json").read_text())
        version = installed["apps"]["effectcraft"]["version"]
        tool = home / "effectcraft" / version / "effectcraft-cli"
        if tool.is_file():
            return str(tool)
    except (OSError, ValueError, KeyError):
        pass
    return "effectcraft-cli"


class NativeProject:
    def __init__(self, path, executable=None):
        self.path = Path(path).resolve()
        self.executable = executable or effectcraft_cli()
        self.receipts = []

    def call(self, *args):
        result = subprocess.run(  # noqa: S603 -- Native CLI argv, without a shell.
            [self.executable, *map(str, args)],
            capture_output=True,
            text=True,
            timeout=180,
        )
        if result.returncode:
            raise RuntimeError((result.stderr or result.stdout).strip())
        return json.loads(result.stdout)

    def edit(self, commands, *, new=False, destination=None):
        commands = list(commands)
        if not commands:
            return []
        # One argument per line preserves exact JSON and avoids the shell/ARG_MAX.
        with tempfile.TemporaryDirectory(prefix="effectcraft-commands-") as temporary:
            args = Path(temporary) / "arguments.txt"
            args.write_text(
                "\n".join(
                    part
                    for command, params in commands
                    for part in (command, json.dumps(params, allow_nan=False))
                )
                + "\n"
            )
            prefix = ["--empty"] if new else ["--project", self.path]
            target = Path(destination or self.path)
            result = self.call(
                *prefix, "run", "--args-file", args, "--save-as", target, "--json"
            )
        rows = result["result"]
        if len(rows) != len(commands):
            raise RuntimeError("EffectCraft returned an incomplete command receipt")
        for row in rows:
            value = row.get("result")
            if isinstance(value, dict) and value.get("errors"):
                raise RuntimeError(f"{row['command']}: {value['errors']}")
        self.receipts.extend(rows)
        return [row["result"] for row in rows]

    def command(self, command, **params):
        return self.edit([(command, params)])[0]

    def info(self):
        return self.call("info", "--project", self.path, "--json")

    def properties(self, layer):
        return self.call("props", "-", layer, "--project", self.path, "--json")

    def enum_options(self, layer, path):
        # Older CLI property replies omit enum labels; the native saved UI retains them.
        prop = self.call("get", "-", layer, path, "--project", self.path, "--json")
        node = next(
            (
                n
                for n in nodes(json.loads(self.path.read_text()))
                if n.get("uid") == prop["uid"]
            ),
            {},
        )
        return node.get("ui", {}).get("options", [])

    def inspect(self):
        info = self.info()
        comp = info.get("activeComp")
        layers = []
        for layer in (comp or {}).get("layers", []):
            props = self.call(
                "props", "-", layer["id"], "--flat", "--project", self.path, "--json"
            )
            layers.append(
                dict(
                    id=layer["id"],
                    name=layer["name"],
                    type=layer["type"],
                    properties=props["properties"],
                    clips=self.enum_options(layer["id"], "geometryOptions/animation")
                    if layer["type"] == "3D Model"
                    else [],
                )
            )
        controls = self.call(
            "exec", "essential.list", "--params", "{}", "--project", self.path, "--json"
        )
        errors = self.call(
            "exec", "expr.errors", "--params", "{}", "--project", self.path, "--json"
        )
        return dict(
            source=str(self.path),
            sha256=digest(self.path),
            version=info["version"],
            composition=comp,
            layers=layers,
            controls=controls,
            expression_errors=errors,
        )
