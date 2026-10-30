"""Compile authored pose timing shared by native cutout exporters."""

import math


def pose_beats(library, action, speed=1.0):
    sequence = library.get("sequences", {}).get(action)
    if not isinstance(sequence, list) or not sequence:
        raise ValueError(f"No authored pose sequence for {action!r}")
    fps = float(library.get("fps", 24))
    if not all(math.isfinite(v) and v > 0 for v in (fps, speed)):
        raise ValueError("Pose FPS and speed must be positive and finite")
    beats, cursor = [], 0.0
    for name in sequence:
        if name not in library.get("poses", {}):
            raise ValueError(f"Missing authored pose: {name}")
        hold = float(library["poses"][name].get("hold_frames", 1))
        if not math.isfinite(hold) or hold <= 0:
            raise ValueError(f"Invalid pose hold: {name}")
        beats.append([cursor, name])
        cursor += hold / (fps * speed)
    beats.append([cursor, sequence[0] if library.get("loop", False) else sequence[-1]])
    return beats
