# Magic Studio agent guide

## Choose the right workflow

Start with the user's request. Operate the installed app through `magic` and
`magic-cli`; discover commands rather than inventing them. A greeting, app fix,
project inspection, or lettering edit does not require a full pipeline preflight.

For app work, read [the app agent contract](apps/src-tauri/agent.md) and
[the Magic CLI/MCP skill](apps/magic-cli/SKILL.md).
The four creative areas are Storyboard, Animation, Lyrics, and Studio. Backstage
is the separate observer. [Setup](apps/magic-cli/README.md) is
only needed for an absent installation or an explicitly requested source build.

## Rule Zero — pipeline production

When the request calls for full video production, the Video
Production department agent ([agents/video-production.md](agents/video-production.md))
takes the work: follow the stage contract (`produces`, `tools_available`,
`review_focus`, `success_criteria`, gates) stage by stage. Reuse the
shared Rust engine commands, the installed Craft CLIs, and `.agents/skills/`; do not
replace them with ad hoc provider scripts. [Project context](PROJECT_CONTEXT.md)
maps these sources.

The agent orchestrates; `magic-cli` and the Craft CLIs provide the mechanical steps.
Resume from real checkpoint artifacts. Canonical artifacts—not free-form
messages—carry stage state.

## Reference Video Entry Point

For “make something like this,” inspect
the actual supplied video with available local analysis tools, preserve the source,
and distinguish observed structure/style from creative proposals. Metadata or a
transcript alone is not visual understanding. Surface access blockers rather than
guessing what unseen footage contains.

## Mandatory Preflight

For pipeline work, discover the actual capability envelope in the prepared
source environment: run the relevant CLI `--help` / `commands` (`magic-cli` and the
Craft CLIs) and check required tools and
meaningful fallbacks against what is actually installed. Translate the result
into available capabilities, relevant setup offers, and concrete runtime warnings.
Report `passed`, `degraded`, or `blocked`; do not infer availability, prices,
keys, or model access from documentation.

## Production decisions

Before generation, present the proposed result, pipeline, exact provider/model,
real alternatives, estimated cost, music plan, and stage plan. Identify sample vs
batch work. Ask before material provider/model/runtime changes, dropping approved
content, or replacing real motion with still-led treatment. Never spend or install
merely to make a capability summary look complete.

`decision_log` is append-only. A changed decision reuses the same `category` and
`subject`, records alternatives and why the prior choice was superseded. A
changed asset alone must not leave the record showing the old choice.

## Present Composition Runtimes

For composition work, present available runtimes with brief-specific
strengths/tradeoffs, recommend one, wait for approval, and log all considered
options under `render_runtime_selection`. If a workflow requires one, explain
the constraint and record the rejected alternative rather than silently defaulting.
Lock `render_runtime` at proposal and carry it through `edit_decisions`.
This pipeline choice does not change the desktop Studio's HyperFrames target.

## Composition Authoring Mode — Templated vs Atelier

Present authoring mode separately from runtime and log `composition_mode`.
Templated work reuses scene types for fast drafts/batches. Hero/brand work should
propose atelier: a piece-specific visual language and authored motion, with the
additional iteration/cost explained before approval. Reuse engine knowledge,
not finished creative looks.

## Composition Runtimes

EffectCraft and FilmCraft handle native lettering (`words.letter`), compositing,
and video assembly (`video.assemble`); HyperFrames uses HTML/CSS/GSAP; FFmpeg handles
direct media assembly. Inspect actual runtime availability before promising a path.
Motion-required work must retain real motion. Never silently turn it into a
Ken Burns animatic because the approved renderer/provider failed.

## Escalate Blockers Explicitly

Report the exact attempt, failure, class of problem (auth, provider, tool, or
creative quality), available options, and recommendation. Investigate safely,
but get approval before a substitute changes the agreed production path.
An unknown outcome after timeout is not permission to replay paid work.

## Stages, review, and approval

Read the workflow's `produces`, `tools_available`, `review_focus`,
`success_criteria`, and `human_approval_default`. Preserve canonical artifacts
in their documented shape in the selected production workspace. Review each stage
against its criteria before advancing.

Write stage entry/progress and actual completed units to the checkpoint. Required
human gates remain `awaiting_human` until approved; do not advance dependent work.
Completed/awaiting checkpoints require the canonical artifact. Retain archived
versions, provenance, original wording/timing, actual tool/model metadata, and
budget estimate/reservation/reconciliation. Approval is per gate unless an
explicit recorded authorization covers more.

For workspace conventions and the retained skill map, use
[MAGIC-STUDIO.md](MAGIC-STUDIO.md) and `.agents/skills/`.
Keep generated projects/media outside the repository. Verify actual saved output
and pixels before claiming success; a receipt is not visual or creative approval.

## Development

Follow [CONTRIBUTING](CONTRIBUTING.md). Preserve intentional deletions, user edits,
component notices, and third-party skills. Do not create behavioral tests before
Jesse observes and explicitly approves the workflow. Report commands actually run
and exact remaining limitations; do not label proposed or unavailable checks as
passed. Original authorship and licensing live in
[SOURCE-ATTRIBUTION.md](SOURCE-ATTRIBUTION.md) and [LICENSE](LICENSE).
