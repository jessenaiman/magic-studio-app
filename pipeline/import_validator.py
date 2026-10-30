#!/usr/bin/env python3
"""
Step 1B: Image Import Validator (`pipeline/import_validator.py`).

Uses `photocraft-cli info` to inspect imported image dimensions, alpha, and layout
without custom pixel flood-fill loops.
"""

from __future__ import annotations

import json
import os
from typing import Any, Dict

from PIL import Image
from pipeline.intake_router import inspect_image

STANDARD_TEMPLATE_SPEC = {
    "templateVersion": "1.0.0",
    "description": "Standard No-Guesswork Character Import Contract (Style-Agnostic)",
    "requiredTurnaroundRow": {
        "views": [
            "0_front",
            "45_three_quarter_front",
            "90_side_profile",
            "135_three_quarter_back",
            "180_back",
        ],
        "minimumRequiredViews": ["0_front", "90_side_profile", "180_back"],
        "baselineAlignmentMaxDriftRatio": 0.08,
        "sideProfileMinWidthRatio": 0.22,
        "background": "Solid uniform studio background or transparent PNG alpha",
    },
    "optionalPoseOrPartRows": {
        "actionPoses": [
            "idle",
            "walk_contact",
            "walk_passing",
            "run_stride",
            "jump_apex",
            "wave_express",
        ],
        "explodedParts": [
            "head",
            "torso_base",
            "costume_skin",
            "arm_L",
            "arm_R",
            "leg_L",
            "leg_R",
            "prop_back",
        ],
    },
}


def validate_imported_asset(image_path: str) -> Dict[str, Any]:
    """Inspect an imported image via `photocraft-cli info` and classify sheet layout."""
    if not image_path or not os.path.exists(image_path):
        return {
            "valid": False,
            "status": "file_not_found",
            "imagePath": image_path,
            "message": f"File not found: {image_path}. Pass any .png/.jpg/.webp path.",
            "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
        }

    pc_info = inspect_image(image_path)
    with Image.open(image_path) as im:
        has_alpha = "A" in im.getbands()
    w, h = pc_info["width"], pc_info["height"]

    return {
        "valid": False,
        "status": "layout_unmeasured",
        "imagePath": os.path.abspath(image_path),
        "dimensions": [w, h],
        "hasAlpha": has_alpha,
        "rowsDetected": None,
        "viewsInPrimaryRow": None,
        "photocraftInfo": pc_info,
        "standardTemplateSpec": STANDARD_TEMPLATE_SPEC,
    }


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("image")
    print(json.dumps(validate_imported_asset(parser.parse_args().image)))
