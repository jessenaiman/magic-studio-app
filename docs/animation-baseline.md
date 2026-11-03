# Animation iteration baseline — 2026-11-03

Jesse accepted the working demo as good enough to iterate on. This is a usable
baseline, not a claim that every character, frame, or collision is approved.

## Preserve the design

- Keep the original Animation workspace, artwork tools, action buttons and timeline.
- Put 3D review inside the existing Scribbles dialog, alongside drawing/sheets.
- Do not introduce another editor or studio page.
- Rust engine commands own option definitions, recipes, validation and persistence.
- TypeScript connects existing controls and renders Three.js previews.
- CSS owns appearance only. Do not move application logic into CSS.

## Existing assets

Nelly's active animation cast is Hayley, Scout, Whiskers and Puddles.
The old cast connections are backed up; collected source meshes remain intact.
Scout and Whiskers are candidate 65-joint Mixamo rigs with 16 downloaded clips.
Hayley and Puddles each retain 21 clips, including their original actions.

Scout and Whiskers used a fitted skeleton, a temporary voxel binding proxy and
weight transfer to the detailed mesh. A hidden donor helper polluted the first
fitting bounds; those candidates were rejected. Check meaningful limb influence,
not merely whether every vertex has a weight.

The external character_pipeline.py wrapper remains experimental: its end-to-end
smoke run stopped at a GLB vertex-count discrepancy. Do not silently weaken this
gate. Serialized vertex counts alone cannot establish mesh parity after export.

## One subject at a time

- Use Whiskers for the next comparison. No batch regeneration.
- Reuse the working GLB; do not re-rig to test compositor timing.
- Announce expensive work, expected scope, resolution and resource limits first.
- Keep jobs serial, with conservative CPU affinity/thread limits for rendering.
- No generation service, credits or model rebuilding unless explicitly requested.
- Keep numerical checks separate from visual approval.
- Make new candidates; never overwrite the accepted baseline.

## EffectCraft then FilmCraft

The installed EffectCraft CLI imports the animated GLB and exposes:

- geometryOptions/animation — embedded clip selection
- geometryOptions/loopAnimation — loop playback
- geometryOptions/animationSpeed — clip speed percentage
- transform properties — stage movement and orientation

The command catalog also exposes keyframe interpolation and smoothing. These
are compositor controls; they are not proof of editable skeletal collision IK.

animation.motion_options lists the Rust-owned options.
animation.prepare_motion creates a saved, single-character EffectCraft project
and its exact CLI recipe, without rendering. Preparation does not block on a
perfect rig or imply the movie has been exported.

Verified: one 640×360 Whiskers clapping test rendered for two seconds at 30 fps
(60 frames) in about 9.5 seconds, restricted to two CPU cores. FilmCraft imported
it into a separate evaluation project and a 60-frame sequence. Nelly’s movie
timeline was not changed. Next, reuse video.assemble / video.export for the full
movie handoff; avoid another parallel export implementation.

## Environment

Speedy is an ext4 boot mount from /etc/fstab and is currently mounted.
EffectCraft MCP additionally needs an automation read/write root for this project.
Mounting a disk does not grant that application permission.
Rigify and GPL CloudRig are present locally. Auto-Rig Pro is paid and is not a
required dependency. Do not promise an addon fixes motion without testing it.

## Reproduce option preparation

Use the installed magic-cli with Nelly as the selected project, then run
`animation-options/evaluate-whiskers.magic.jsonl` from that project folder.
The JSONL calls animation.motion_options and prepares the baseline and paced
options serially for one character. It performs no rendering.

## Alpha ownership decision

Keep the existing Animation stage and transport as the entry point. Konva owns the
2D artwork canvas. Three.js and GLTFLoader own the 3D canvas and embedded clip
sampling. The transport scrubs source time. Theatre core/studio owns authored
2D sheet keyframes. Rust persists Theatre snapshots in the project document.
Embedded GLB tracks are sampled by Three.js, with pose corrections and revisions
in Scribbles review. An embedded clip is distinct from an authored Theatre sheet.

Scribbles owns reference drawings, generated sheets, and visual review through
the existing artwork editor and scribbles engine commands. Fidget owns chat and
assistant interaction through the agent panel and engine agent commands. Tauri
hosts the panels. CSS controls their appearance.

Prefer composing those existing owners over a unified replacement editor.
Keep Tripo output in the character's 3d folder, inspect the imported file, and
offer its actual embedded clips in the Animation canvas. Preset requests alone
are insufficient evidence that a playable clip exists. Alpha acceptance requires
a real asset, clip playback and seeking, revision round trip, and movie handoff.
