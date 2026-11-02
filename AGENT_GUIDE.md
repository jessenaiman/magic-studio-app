# Magic Studio agent guide

## Choose the right workflow

Start with the user's request. Operate the installed app through `magic` and
`magic-cli`; discover commands rather than inventing them. A greeting, app fix,
project inspection, or lettering edit does not require a full pipeline preflight.
Use [onboarding](skills/meta/onboarding.md) only for a genuinely open-ended start.

For app work, read [the app agent contract](apps/src-tauri/agent.md) and
[the Magic CLI/MCP skill](apps/magic-cli/SKILL.md).
The four creative areas are Storyboard, Animation, Lyrics, and Studio. Backstage
is the separate observer. [Setup](apps/magic-cli/README.md) is
only needed for an absent installation or an explicitly requested source build.

## Rule Zero — pipeline production

When the request calls for the reusable production toolkit, the Video
Production department agent ([agents/video-production.md](agents/video-production.md))
takes the work: choose the actual `pipeline_defs/<pipeline>.yaml` preset, read it
before execution, and follow its stage entries (`produces`, `tools_available`,
`review_focus`, `success_criteria`, gates) stage by stage. Reuse the
`pipeline/` scripts, YAML presets, and skills; do not replace the preset with
ad hoc provider scripts. [Project context](PROJECT_CONTEXT.md) maps these sources.

The agent orchestrates; the `pipeline/` scripts provide the mechanical steps.
Resume from real checkpoint artifacts. Canonical artifacts—not free-form
messages—carry stage state.

## Reference Video Entry Point

For “make something like this,” read
[video reference analysis](skills/meta/video-reference-analyst.md) first. Inspect
the actual supplied video with available local analysis tools, preserve the source,
and distinguish observed structure/style from creative proposals. Metadata or a
transcript alone is not visual understanding. Surface access blockers rather than
guessing what unseen footage contains.

## Mandatory Preflight

For pipeline work, discover the actual capability envelope in the prepared
source environment: run the relevant CLI `--help` / `commands` (`magic-cli`, the
Craft CLIs, `cli-anything-blender`) and check the preset's `required_tools` and
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

## Present Both Composition Runtimes (HARD RULE)

For composition work, when Remotion and HyperFrames are both
available, present each with brief-specific strengths/tradeoffs, recommend one,
wait for approval, and log all considered options under
`render_runtime_selection`. If a pipeline requires one, explain the constraint
and record the rejected alternative rather than silently defaulting.
Lock `render_runtime` at proposal and carry it through `edit_decisions`.
See [the runtime matrix](skills/core/hyperframes.md) and
[animation runtime selection](skills/meta/animation-runtime-selector.md).
This pipeline choice does not change the desktop Studio's HyperFrames target.

## Composition Authoring Mode — Templated vs Atelier

Present authoring mode separately from runtime and log `composition_mode`.
Templated work reuses scene types for fast drafts/batches. Hero/brand work should
propose atelier: a piece-specific visual language and authored motion, with the
additional iteration/cost explained before approval. Reuse engine knowledge,
not finished creative looks. Read [taste direction](skills/meta/taste-direction.md)
and [bespoke composition](skills/meta/bespoke-composition.md).

## Composition Runtimes

Remotion uses the React composition stack; HyperFrames uses HTML/CSS/GSAP;
FFmpeg handles direct media assembly. Inspect actual runtime availability and
[scene contracts](remotion-composer/SCENE_TYPES.md). Do not promise parity where a
pipeline depends on a runtime-specific composition or caption path.
Motion-required work must retain real motion. Never silently turn it into a
Ken Burns animatic because the approved renderer/provider failed.

## Escalate Blockers Explicitly

Report the exact attempt, failure, class of problem (auth, provider, tool, or
creative quality), available options, and recommendation. Investigate safely,
but get approval before a substitute changes the agreed production path.
An unknown outcome after timeout is not permission to replay paid work.

## Stages, review, and approval

Read the manifest's `produces`, `tools_available`, `review_focus`,
`success_criteria`, and `human_approval_default`. Preserve canonical artifacts
in their documented shape in the selected production workspace. Use
[reviewer](skills/meta/reviewer.md) for stage self-review; do not duplicate its
full policies here.

Write stage entry/progress and actual completed units to the checkpoint. Required
human gates remain `awaiting_human` until approved; do not advance dependent work.
Completed/awaiting checkpoints require the canonical artifact. Retain archived
versions, provenance, original wording/timing, actual tool/model metadata, and
budget estimate/reservation/reconciliation. Approval is per gate unless an
explicit recorded authorization covers more.

For source pipeline workspace conventions and the retained skill map, use
[MAGIC-STUDIO.md](MAGIC-STUDIO.md) and [skills/INDEX.md](skills/INDEX.md).
Keep generated projects/media outside the repository. Verify actual saved output
and pixels before claiming success; a receipt is not visual or creative approval.

## Development

Follow [CONTRIBUTING](CONTRIBUTING.md). Preserve intentional deletions, user edits,
component notices, and third-party skills. Do not create behavioral tests before
Jesse observes and explicitly approves the workflow. Report commands actually run
and exact remaining limitations; do not label proposed or unavailable checks as
passed. Original authorship and licensing live in
[SOURCE-ATTRIBUTION.md](SOURCE-ATTRIBUTION.md) and [LICENSE](LICENSE).
