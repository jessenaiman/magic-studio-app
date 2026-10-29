# Magic Studio production guide

Start with the [overview](README.md), [setup](agent-harness/cli_anything/magic/README.md),
and [current command skill](agent-harness/cli_anything/magic/skills/SKILL.md).
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
`rig.from_sheet` cuts parts, reconstructs the original placement, and saves the
rig plus review images. Inspect those images and joints before claiming a usable
articulated rig. `motion.place_section` places a layered section only after its
motion gate succeeds. Discover the current parameter contracts before use.

Blender mode runs renders in a separate resource-limited worker. The same workflow
is exposed by `blender.start`, `blender.status`, `blender.cancel`, and `blender.list`.
Editable packed `.blend` scenes retain subsequent work. See
[Blender animation](docs/workflows/blender-animation.md) and
[reference → animation → video](docs/workflows/reference-to-animation-video.md)
for the actual handoff and remaining manual steps.

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

Sproket uses `agent.send`, `agent.log`, and `agent.session` in the selected project.
Select the intended window first; poll the existing request rather than resending
it. `agent.new_chat` explicitly creates a fresh conversation and archives history.
Model/provider selection belongs to that Studio instance, separate from shared
credentials/defaults. Gaffer artwork uses the separate image operation.

[Backstage](apps/backstage/README.md) observes saved project references and
engineering evidence; it does not control sessions. [Backlot](backlot/README.md)
is the pipeline artifact/checkpoint board, not the native storyboard editor.

## Reuse the production toolkit

The source repository retains `tools/`, `lib/`, `pipeline/`, `pipeline_defs/`,
`skills/`, `schemas/`, `styles/`, and the composition adapters. These are working
building blocks for agent-directed production, not a second installation path.
Read [the agent guide](AGENT_GUIDE.md) and the selected YAML/stage skills before
using them; preflight actual dependencies and provider access in the prepared
source environment.

Pipeline stages exchange canonical artifacts and checkpoints. Composition may
use approved Remotion, HyperFrames, or FFmpeg paths according to the pipeline;
the desktop Studio remains HyperFrames. Preserve approval gates, source credit,
provenance, cost records, and editable assets when adapting a workflow.

[License](LICENSE) · [Source attribution](SOURCE-ATTRIBUTION.md)
