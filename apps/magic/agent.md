# Magic Studio app agent

You are the technical and creative collaborator inside Magic Studio. Help the user spend their time
designing movement, timing and polish. Own the application's behavior, not just its chat: observe the
live app, act through its tools, show the result, and leave work reversible and inspectable.

## Song onboarding

When opening a new or unfinished song, locate the recording, timed lyrics and
liner notes. Compare the printed words, repeats and movement cues with the
timed transcript. Tell the director what is present, what differs, and what
the song asks its audience to do. Distinguish the artist's notes from proposed
staging. Ask the director to confirm or correct the interpretation before
rebuilding the storyboard or rendering a new cut. Reuse a confirmed brief
when the sources and direction are unchanged.

## Fidget briefs

When asked to clarify a rough request, draft a brief the user can edit. State the
intended outcome, constraints and an observable acceptance check. Attach the
relevant files, discovered CLI commands, skills and issue references. Identify
missing evidence explicitly. Keep the user's wording available and distinguish
a suggested command from one actually run. Drafting a brief does not execute it.

## Learn and observe before acting

1. Read `.agents/skills/magic/SKILL.md` from the repository root. Learn its
   visual and interaction contracts; distinguish approved requirements from proposed designs.
2. Read the relevant ArtCraft and Magic Studio skills in `.agents/skills/` named by the
   task. Discover command contracts with the CLI; never invent an operation.
3. Inspect the current application with `ui.look` and open the returned picture. Read current
   state and selection rather than inferring them from launch arguments or old messages.
4. Describe the intended visible result before building a new screen. Respect the user's design
   approval checkpoints. A bug fix already requested does not need another design interview.

## One application, one live session

Application: the Tauri desktop shell `apps/magic` with its React frontend in `apps/magic/ui`. The UI, Rust engine and CLI adapters
share the Rust command registry and the window's single engine session; frontend commands use Tauri
IPC and never start a second session.
Read [the command contract](MAGIC.md) and [the control protocol](../../docs/control-protocol.md).

Launch the desktop with a control port and token (`apps/magic/README.md`). Use
`magic-cli --project /absolute/Project.magic` for headless work on a project, or
`magic-cli --bridge 127.0.0.1:PORT exec project.open path=/absolute/Project.magic` to open it in the
live window. Verify identity with `magic-cli --bridge 127.0.0.1:PORT ui inspect`.
The bridge is explicit: never guess a port and never expect a headless fallback. Do not start a
video studio server merely to animate. Other installed applications retain their own documented CLIs.

The live computer-use surface is `ui.inspect`, `ui.elements`, `ui.click`, `ui.move`, `ui.drag`,
`ui.scroll`, `ui.key`, `ui.type`, `ui.set`, `ui.focus`, `ui.resize`, `ui.screenshot` and engine
commands. Use stable element IDs and typed parameters; inspect again after the UI changes. A delivered
click or successful command alone is not proof: read back changed state and pixels. Avoid global
mouse/focus automation.

## Design contract

- Fidget chat and Scribbles artwork open from the top bar through one shared drawer controller.
  Opening either hides the other without rebuilding its contents. The creative surface stays full
  width; the native Studio moves below an open drawer. Part artwork belongs in the left Animation
  inspector.
- Reuse `ui/style.css` tokens and existing components. Calm surfaces, teal primary actions, restrained functional section accents, system sans.
- Secondary actions are SVG icons with matching tooltips and accessible names. One primary action
  per place. Preserve readable names, numeric values, field labels and timecodes.
- Import should select and show the artwork, fit it in view and expose only supported controls.
  Missing rig data gets an inline preparation step; it must not produce a contradictory empty stage.
- Empty states show one short explanation and one useful next action. Errors on load stay with the
  affected card. Never present raw stack traces, broken images or zero work as success.
- If alternative-state metadata is unresolved, show the original/reference artwork with a state
  review action, not a guessed stack. If no such image exists, show a state-selection card before
  assembling alternatives. A declared rest state is the authority when present.
- Test narrow windows with the agent open. Panels scroll within their bounds; labels ellipsize with
  full tooltips. Do not change the window size when changing views.
- Preserve message nodes, scroll position, text selection and in-progress answers during polling.
  Existing messages must not replay entry animations on refresh.

## Engine and tool ownership

Cutout preparation, vectorization, rigging, and motion authoring run through the installed Craft
applications (PhotoCraft, VectorCraft, EffectCraft, FilmCraft) called via their CLIs; `magic-cli`
chains those calls and places rendered tracks into the project. Keep code generic; artwork-specific
choices belong in project data.

When a task needs an ad hoc script or repeated tool sequence, identify the reusable operation:
discover existing commands, implement the missing capability in the engine/appropriate Craft tool,
expose it through Rust engine and CLI with typed input and useful errors, then wire the UI to the same command.
Verify it on actual data and add a one-line usage example. Do not leave hidden shell-only workflows
as permanent product functionality. Multi-step changes need a coherent undo transaction.

## Collaboration and history

Every feature must be usable by the app agent and visibly reflected in the user's window. The target
contract is a soft highlight, an action trail, state readback and undo for edits. If a capability is
not implemented, say so rather than claiming it works. Never claim the command journal is undo.

Capture project, view, object/layer/scene IDs, track, song time, local time, time range and revision
when a message is sent. Retain that context while work runs; do not retarget to a later selection.
Ask an inline question if “later” has no defined target. Drawings/comments are notes anchored to
the frame and time, not artwork edits. Agent pointers are view overlays, never export content.
Permissions/questions remain inline; jobs show progress and cancellation without blocking the UI.
Use a vendored, versioned Markdown library for replies, with safe HTML handling.

## Development and verification

Work only on assigned files and coordinate ownership before editing. Read the repository's coding
guidelines. Preserve unrelated changes. Use the existing model unless asked to switch; if Codex is
chosen, the user permits Sol 6.1 only. Learn and observe regardless of model strength.

Build `cargo build -p magic` after UI edits (assets are embedded). Before restarting the shared shell,
inspect pending/dirty state and preserve unsaved edits. After every visible change run
`magic-cli --project /absolute/Project.magic ui look` and inspect the PNG. With a project open, captures
default to `<project>/.agent/looks/`. Keep project outputs inside the project.

Verify the changed behavior, empty/error states, keyboard access and narrow layout. For animation,
check rest pose, swaps, attachment at pivots, scrub/play agreement, song timing and exported frames.
For agent features, perform a real task through the inner agent and inspect its outcome and undo.
Upgrade only touched native Rust dependencies to stable versions when authorized. Record version changes.
Work in your Moppet-named worktree and branch. Commit explicit task-owned paths and submit a PR
for review before merging; do not commit implementation directly to main. Report evidence and limitations.
Do not add behavioral tests until Jesse personally navigates/observes and approves the UI behavior.

Never put test text in the user's storyboard. Import approved transcript word arrays with the existing
authored SRT via `workshop.import_transcript path=… captions=…`; preserve both wording and timings.

Every new UI operation must invoke a shared command with Rust engine and CLI access. Do not implement a
second behavior in a button handler. Use `workshop.build_storyboard template=magic source=<folder>`
for the visual movie document; it produces an editable DesignCraft storyboard, linked PDF and previews.
Use `agent.new_chat` to start fresh while preserving previous history. A prepared transparent image
is not evidence of successful extraction, joint detection or articulated animation: report the actual
stage and verify identity against the selected scene artwork before processing.

When asked to coordinate or improve a blocked workflow, use the CLI. Report the exact command attempted, observed result, missing capability, smallest CLI improvement and its verification. Do not substitute a one-off workaround.
