"""Author Puppet rigs and captured paths using EffectCraft's own motion tools."""

try:
    from .effectcraft_project import fields, nodes
except ImportError:
    from effectcraft_project import fields, nodes


class PuppetRig:
    def __init__(self, project, layer):
        self.project, self.layer = project, layer
        self.pins = {}

    def build(self, spec):
        fields(spec, "pins mesh recordings follow", "Puppet")
        fields(spec.get("mesh", {}), "density expansion triangles", "Puppet mesh")
        mesh = None
        for pin in spec.get("pins", []):
            fields(pin, "id kind position keys expression scale rotation amount extent inFront", "pin")
            name = pin["id"]
            if name in self.pins:
                raise ValueError(f"Duplicate pin ID: {name}")
            options = dict(spec.get("mesh", {})) if mesh is None else {"mesh": mesh}
            result = self.project.command(
                "puppet.addPin", layer=self.layer, kind=pin.get("kind", "position"),
                position=pin["position"], newMesh=mesh is None, **options
            )
            mesh, uid = result["mesh"], result["pin"]
            self.pins[name] = uid
            self.project.command("prop.renameGroup", layer=self.layer, prop=uid, name=name)
            settings = {k: pin[k] for k in ("scale", "rotation", "amount", "extent", "inFront") if k in pin}
            commands = [("puppet.setPin", dict(layer=self.layer, pin=uid, **settings))] if settings else []
            commands.extend(("puppet.movePin", dict(layer=self.layer, pin=uid, time=t, position=value))
                            for t, value in pin.get("keys", []))
            if "expression" in pin:
                commands.append(("prop.setExpression", dict(
                    layer=self.layer, path=self.position_path(uid), expression=pin["expression"])))
            self.project.edit(commands)
        self.record(spec.get("recordings", []))
        self.follow(spec.get("follow", []))
        return dict(mesh=mesh, pins=self.pins)

    def position_path(self, uid):
        pin = next((n for n in nodes(self.project.properties(self.layer)) if n.get("uid") == uid), {})
        path = next((c["path"] for c in pin.get("children", []) if c.get("match") == "position"), None)
        if not path:
            raise ValueError("This pin does not expose an animatable position")
        return path

    def record(self, recordings):
        for recording in recordings:
            fields(recording, "pin samples pins start speed smoothing", "pin recording")
            params = dict(recording, layer=self.layer, pin=self.pins[recording["pin"]])
            if "pins" in params:
                params["pins"] = [self.pins[name] for name in params["pins"]]
            self.project.command("puppet.recordPin", **params)

    def follow(self, followers):
        for follow in followers:
            fields(follow, "leader pins delay amount cascade", "follow-through")
            self.project.command("puppet.follow", **dict(
                follow, layer=self.layer, leader=self.pins[follow["leader"]],
                pins=[self.pins[name] for name in follow["pins"]]))


def animate(project, layer, spec):
    commands = []
    for path, settings in spec.get("properties", {}).items():
        fields(settings, "value keys expression", "property animation")
        if "value" in settings:
            commands.append(("prop.set", dict(layer=layer, path=path, value=settings["value"])))
        commands.extend(("prop.addKey", dict(layer=layer, path=path, time=t, value=value))
                        for t, value in settings.get("keys", []))
        if "expression" in settings:
            commands.append(("prop.setExpression", dict(layer=layer, path=path, expression=settings["expression"])))
    if "sketch" in spec:
        fields(spec["sketch"], "points start captureSpeed smoothing", "motion sketch")
        commands.append(("motion.sketch", dict(spec["sketch"], layer=layer)))
    project.edit(commands)
    return PuppetRig(project, layer).build(spec["puppet"]) if "puppet" in spec else None
