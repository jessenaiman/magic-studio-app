---
name: image-to-animation
description: Use when carrying out image-to-animation work in Magic Studio with user-supplied artwork, pose sheets, turnaround views or part sheets.
---

# Image-to-animation

## Establish the requested result

Use the user's current request and confirmed decisions as the requirements. Identify the supplied artwork and requested output. Ask when an unresolved choice changes that output; do not assume a character, art style, anatomy, number of views, action sequence, export target or approval checkpoint.

Inspect the actual images and any accompanying manifests. Treat labels, poses and view angles as evidence to verify, not facts to infer from frame numbers. Preserve missing information as missing.

Read what is already registered in the project and already on disk before proposing steps, and confirm the character roster with the user rather than taking one from a preset list or a previous task.

## Check the current tools

Inspect the installed entry points and their current help before choosing commands. Verify capabilities by their behavior and produced artifacts; registration in a command catalogue or a successful build alone does not prove a feature works.

Use the shared Magic registry through `magic-cli`. Inspect `magic-cli commands asset.cut_sheet`,
`magic-cli commands turnaround.optimize_accuracy` and the relevant rig command before running them.
Select the project with `magic-cli project open path=/absolute/Project.magic`. Use one
`magic-cli run` JSONL batch for stages that depend on the active asset; separate calls do not retain
transient engine state. `image.generate rig_parts=` and `rig.from_sheet` have their own generation
receipt contract; do not mix that route with image-agent sheets without checking the inputs.

Use only the operations needed for the requested result. Inspect other available tools when the task needs different functionality. If a required operation fails or is unavailable, report the concrete gap and ask about alternatives that would change the result. Do not invent missing implementation or silently substitute another deliverable.

## Process the supplied assets

Use actual registered asset IDs, explicit sheet kinds and output locations appropriate to the task.
The registry exposes these parameter forms:

```bash
magic-cli run <<'JSONL'
{"id":"project.load_asset","params":{"id":"REGISTERED_ASSET"}}
{"id":"asset.cut_sheet","params":{"asset":"REGISTERED_ASSET","kind":"turnaround","out_dir":"/absolute/cutouts"}}
JSONL
magic-cli exec turnaround.optimize_accuracy asset=REGISTERED_ASSET \
  turnaround_dir=/absolute/turnaround poses_dir=/absolute/poses out_dir=/absolute/registered
```

Replace example IDs and paths with the selected inputs before execution. Run the optimizer only
when registration or motion measurement is requested and its required inputs exist. Read returned
summaries and manifests for frame paths, labels, matting engine, missing inputs and review status.
Choose layout and refinement settings from the actual images and supported command parameters.

Check exit status and inspect the produced files. Review cutout outlines and any requested part masks against the source. Correct observed errors with the supported layout/mask controls and rerun the affected stage. Preserve source artwork; identify output revisions clearly.

## Verify and hand over

Check the current output schema and actual artifacts against the requested result. For `turnaround.optimize_accuracy`, check the returned `registration_passed` and
`max_vertical_drift_px`, then inspect the registered images and `motion_range.json`; report missing
poses and unavailable values. A result at one alpha threshold establishes that measured image alignment only, not anatomical correspondence or seamless 3D geometry. Observed silhouette changes do not establish joint-angle limits or a temporal animation cycle.

Use manifest paths, labels and recorded transforms when connecting consumers. Do not apply registration twice or reuse untransformed part pivots on transformed images. Test the consumer path actually used and distinguish generated files from verified application behavior.

Return the resulting artifact paths, what was executed and inspected, and any unresolved part of the user's request. Keep these instructions synchronized with verified tool behavior as capabilities change; do not add speculative commands or turn an implementation limitation into a permanent user requirement.
