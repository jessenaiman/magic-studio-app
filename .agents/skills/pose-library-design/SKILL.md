---
name: pose-library-design
description: Design reusable 2D character pose sheets, 8-frame action sprite cycles, and pose-to-pose EffectCraft performances using Scribbles templates.
license: MIT
---

# Pose Library Design

Use this skill when generating character pose sheets (`scribbles.poses`), consecutive action cycles (`scribbles.actions`), or scripting pose-to-pose character acting in EffectCraft (`.ecproj`).

## Scribbles Sheet Contracts (`crates/engine/src/scribbles/templates.json`)

1. **Pose Sheet (`scribbles.poses` — `1536×1024`, $3 \times 2$ grid of `512×512` cells)**:
   - Six full-body acting poses read left-to-right, top-to-bottom (e.g., `idle`, `walk/open`, `strain/jump`, `action/wave`, `point/talk`, `wave/cheer`).
   - Every figure is ~440 px tall with planted feet sharing the cell baseline at `y = 490 px` from the cell top (`y = 476..483` top row, `y = 984..993` bottom row).
   - **Never crop or slice a single pose cel into polygon limbs.** Import the sheet into EffectCraft, set the layer `anchor` to the planted feet coordinates of that cell, apply a `512×512` cell mask (`mask.new`), and swap between pose cels on acting beats.

2. **Action Frames Sheet (`scribbles.actions` — `1536×1024`, $4 \times 2$ grid of `384×512` cells)**:
   - Eight consecutive frames of a single cyclical action (`walk`, `run`, `jump`, `wave`, `turn`) with the planted foot at `y = 490 px` from the cell top and identical horizontal center so frames play in place via `character.sprite` (`loopOut("cycle")`).
   - Key breakdowns:
     - `walk`: contact (right foot forward), down, passing, up, contact (left foot forward), down, passing, up.
     - `run`: contact, down, push-off, flight, contact (other foot), down, push-off, flight.
     - `jump`: stand, anticipation crouch, take-off, rise, apex, fall, landing squash, recover.
     - `wave`: rest, arm rising, hand up open, wave left, wave right, wave left, wave right, arm lowering.

## Pose-to-Pose Acting Pattern in EffectCraft

Use timed pose sequences driven by story beats (`anticipation -> extreme -> overshoot -> settle -> hold`):

1. **Foot-Anchored Placement**: Anchor every pose cel at the character's feet so scale (`squash`) and rotation (`turn`) pivot from the floor contact point instead of floating.
2. **Anticipation & Squash/Stretch**: Before a major action beat or impact, key a 2–4 frame anticipation (`turn` opposite by `5°–8°` or `scale` `[109, 116]`), snap to the impact (`scale` `[119, 103]`), and settle back to `[112, 112]`.
3. **Readable Holds**: Do not continuously tween every limb with sine waves. Hold key poses so the silhouette and facial expression read clearly against the scene and lyrics.

## Reference Scripts in This Repo

- [`projects/make-it-sound-human/motion/authoring/drawn_cut.py`](../../../projects/make-it-sound-human/motion/authoring/drawn_cut.py)
- [`projects/make-it-sound-human/motion/authoring/drawn_acting.py`](../../../projects/make-it-sound-human/motion/authoring/drawn_acting.py)
- [`projects/make-it-sound-human/motion/authoring/drawn_shots.py`](../../../projects/make-it-sound-human/motion/authoring/drawn_shots.py)

## Sources

- School of Motion — *Walk Cycle in After Effects & Pose-to-Pose Blocking*: https://schoolofmotion.com/blog/walk-cycle-after-effects
- Esoteric Software Spine — *How to cut your assets for animation*: https://esotericsoftware.com/blog/How-to-cut-your-assets-for-animation
