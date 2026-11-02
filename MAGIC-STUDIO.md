# Magic Studio production guide

Start with the [overview](README.md), [setup](apps/magic-cli/README.md),
and [current command skill](apps/magic-cli/SKILL.md).
Discover each operation with `magic-cli commands <domain>`; command results and
saved native output are the authority, not historical handoff documents.

## 1. Storyboard: bring in the song and plan the scenes

Create/open a project through the app's File menu. New song starts in Storyboard;
Home and Ctrl+N use the same workflow. Save original media in the production
folder selected in Application Settings. That setting does not move old projects.

Bring existing timed lyrics and SRT rather than retranscribing them.
`workshop.import_transcript` accepts timed JSON with optional original SRT via
`captions`. Preserve every word and timestamp; distinguish raw transcription
from corrections. The shared reader prefers `hyperframes/transcript.json` and
retains legacy source files.

For deliberately automated intake, discover `project.intake` and supply only
inputs you have. It reuses valid supplied timing; otherwise it invokes the
existing audio analysis. Inspect `intake_ready`, `storyboard_ready`, `steps`,
`problems`, and `next_commands`: `ok:true` only means the request was handled.
A failed step leaves the project openable, not ready by implication.

New song's prepared path initializes the official HyperFrames audio project and
native DeckCraft storyboard template; transcription, scene creation, and artwork
placement remain explicit. `storyboard.template` reports the canonical template.
The deck is editable; its PDF and previews are exports, not editable slide objects.
`storyboard.import_magicstudio path=/existing/project` imports labelled scene
content from an existing production folder.

`workshop.build_storyboard draft=true` creates a draft for review.
`storyboard.save_version draft=<manifest.json>` adopts it. Native text editing and
`storyboard.edit` fill the deck fields. `storyboard.design` reports missing content
and the current deck/PDF paths. Inspect those missing fields and the actual scene
content before proceeding. A saved draft or an approval button is not evidence
that the current document has received a complete creative review.

## 2. Animation: prepare artwork and motion

Use `image.generate` for artwork and reference-driven action sheets; receipts
retain source artwork and storyboard context. It uses the logged-in native image
tool through a one-shot Codex session. Actual dimensions are returned; do not
promise a fixed resolution or treat the selected Codex model as the image model.
Only receipt-correlated, decoded PNGs are attached to the project. Keep failure
logs and partial files distinguishable from a successful asset.

Rig-sheet generation uses `pipeline/templates/rig_sheet_spec.json`.
Cutout preparation, vectorization, rigging, and motion authoring run through the
installed Craft applications (`photocraft-cli`, `vectorcraft-cli`,
`effectcraft-cli`, `filmcraft-cli`). Inspect cutout quality, joints, and rendered
frames before claiming a usable rig or animation. Rendered transparent clips are
placed into the composition with `video.place_track`. Discover the current
parameter contracts with `--help` and `commands` before use.

## 3. Lyrics: design the song's lettering

`words.design` uses the project's `lyrics-design.json` to design sung lines.
Recipes define palette, band, base lettering, and hero-word styling/motion.
Rendered fonts are measured to keep lines within frame. The caption track retains
sections without a design. Fine-tune individual lines in Lyrics afterwards.
Preserve the source lyrics and timing while changing visual treatment.

## 4. Studio: compose and inspect the video

The desktop Studio targets HyperFrames. `video.init target=hyperframes` invokes
its official initializer and imports supplied native timing without another ASR
run. Existing workspaces are reused; scaffold replacement is rejected. Audio and
timing import does not constitute authored visuals or a finished video.

The app resolves an explicit `hyperframes_cli`, `HYPERFRAMES_CLI`, or the installed
`hyperframes` on PATH. It does not install an unversioned runtime through npx.
Native caption initialization preserves unchanged-input edits/overrides and
reports changed-timing conflicts. Initial word groups remain provisional until
listening and visual review. Keep runtime failures visible rather than replacing
the approved path with a custom pipeline.

HyperFrames edits composition files inside the same project. Contained media,
recording, and workspace paths are stored relative to the `.magic` file and
resolved from its folder on reopen. File > Save includes pending storyboard
edits; `storyboard.save_version` saves the design itself. Confirm saved output
and inspect rendered frames before calling a video complete.

## Project agent and observers

Fidget uses `agent.send`, `agent.log`, and `agent.session` in the selected project.
Select the intended window first; poll the existing request rather than resending
it. `agent.new_chat` explicitly creates a fresh conversation and archives history.
Model/provider selection belongs to that Studio instance, separate from shared
credentials/defaults. Scribbles artwork uses the separate image operation.

[Backstage](apps/backstage/README.md) observes saved project references and
engineering evidence; it does not control sessions.

## Reuse the production toolkit

The source repository retains `pipeline/templates/`, `agents/`, and `.agents/skills/`.
These are working building blocks for agent-directed production, not a second
installation path. Read [the agent guide](AGENT_GUIDE.md) before using them;
preflight actual dependencies and provider access in the prepared source
environment.

Pipeline stages exchange canonical artifacts. Composition uses EffectCraft,
FilmCraft, DeckCraft, or HyperFrames according to the workflow. Preserve
approval gates, source credit, provenance, cost records, and editable assets
when adapting a workflow.

[License](LICENSE) · [Source attribution](SOURCE-ATTRIBUTION.md)
