"""Resolve external artwork locations from explicit configuration."""
from __future__ import annotations

import json
import os
from pathlib import Path


def configured_path(key: str, environment: str) -> Path:
    """An environment override wins over the shared Magic Studio settings file."""
    value = os.environ.get(environment)
    if not value:
        config = os.environ.get("XDG_CONFIG_HOME")
        base = Path(config) if config else Path.home() / ".config"
        settings = base / "magicstudio/settings.json"
        try:
            data = json.loads(settings.read_text())
        except FileNotFoundError:
            data = {}
        if not isinstance(data, dict):
            raise ValueError(f"Invalid Magic Studio settings object: {settings}")
        value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(
            f"Configure {key} in Magic Studio settings.json or set {environment}"
        )
    path = Path(value.strip())
    if not path.is_absolute():
        raise ValueError(f"{key} must be an absolute path: {value!r}")
    return path
