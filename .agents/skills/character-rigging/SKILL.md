---
name: character-rigging
description: Build data-driven 2D character rigs and layered compositions in EffectCraft: open-stance asset prep, rounded joint caps, replacement cels, and subtract/add sub-layer articulation.
license: MIT
---

# Character Rigging

Use this skill when preparing 2D character artwork for rigging or building layered EffectCraft (`.ecproj`) character compositions in Magic Studio.

## Golden Rule: Never Cookie-Cut a Posed Painting

- **NEVER slice polygons out of a single posed 3/4 or hands-on-hips character painting (`character.cut`) to build a rotating skeletal rig.**
- When an arm or hand overlaps the torso in a source painting, polygon-cutting steals torso border pixels, leaves holes behind, and waves a hip-pressed fist in mid-air.
- Instead, choose one of the three legitimate 2D animation workflows below based on the shot requirements:
  1. **Pose-to-Pose Acting Cels (`scribbles.poses` / `Toasty - pose cels.png`)**: Full-body acting poses in a $3 \times 2$ grid (`512×512` cells) anchored at the planted feet, masked non-destructively in EffectCraft (`mask.new`) and animated with anticipation/overshoot/settle (`position`, `rotation`, `scale` squash-and-stretch).
  2. **Frame-by-Frame Action Sprite Cycles (`scribbles.actions` → `character.sprite`)**: 8 consecutive action frames in a $4 \times 2$ grid (`384×512` cells, planted foot baseline at `y = 490 px`) for walk, run, jump, wave, or turn cycles, looped via EffectCraft Time Remapping (`loopOut("cycle")`).
  3. **Skeletal / Layered Rig from an Open Stance or Exploded Parts (`scribbles.image` + Sub-Layer Articulation)**: Only rig separated parts when the character is drawn in an open, non-overlapping stance (`stance` rule in `crates/engine/src/scribbles/templates.json`) or with clean hidden overlap and rounded circular joint caps.

## Asset Preparation Rules (Esoteric Software Spine / 2D Cutout Standard)

1. **Open Animation-Ready Stance**: Feet planted shoulder-width apart with clear background between the legs; arms held ~20° away from the body with clear gap between each arm and torso; hands open; no limb overlapping another limb (`https://esotericsoftware.com/blog/How-to-cut-your-assets-for-animation`).
2. **Complete Hidden Geometry**: Never crop an asset flush at the seam where it meets another part. Paint the full underlying torso/shoulder behind the arm and the full neck beneath the head so rotation never exposes a gap.
3. **Rounded Circular Joint Caps**: Overlapping limb ends (shoulders, elbows, knees) must have circular rounded caps centered on the joint pivot so rotating the limb keeps the silhouette seamless.
4. **Targeted Sub-Layer Articulation**: When a drawn pose cel only needs one articulating limb or prop (e.g., a conducting forearm or spinning chest fan):
   - Apply a subtract mask (`mask.new` with `mode: "subtract"`) around that sub-part on the base pose cel.
   - Add a second layer of the same sheet above it with an add mask (`mask.new`), set its anchor at the anatomical joint socket or rotor hub, and keyframe `transform/rotation`.
5. **Replacement Cels / Slot Swapping**: Use hold-opacity layer swaps (`0%` / `100%`) for hand gestures (`fist`, `open`, `point`, `wave`) and facial expressions (`neutral`, `happy`, `surprised`, `focused`) rather than rotating a static hand drawing into an impossible pose.

## Sources

- Esoteric Software Spine — *How to cut your assets for animation*: https://esotericsoftware.com/blog/How-to-cut-your-assets-for-animation
- School of Motion — *Walk Cycle in After Effects & Pose-to-Pose Blocking*: https://schoolofmotion.com/blog/walk-cycle-after-effects
