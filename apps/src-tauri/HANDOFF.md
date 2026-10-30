# Magic Studio shell handoff

## Current application

The Tauri shell is `apps/magicstudio-shell/`. It embeds four creative areas:
Storyboard, Animation, Lyrics, and Studio. `magic` opens the application;
`magic-cli` and `magic-cli mcp` expose the same registry-owned operations.

Read [the app agent contract](agent.md),
[the command architecture](../../agent-harness/MAGIC.md), and
[the UI skill](../../.agents/skills/magicstudio-shell-ui/SKILL.md) before app work.
[Setup](../../agent-harness/cli_anything/magic/README.md) is the single source for
build and installation. Embedded UI changes need a rebuilt shell.

## Production workflow

- Keep original recordings, lyrics, timestamps, and source artwork in the selected
  project; do not place generated projects/media in the repository.
- Storyboard uses the canonical editable DeckCraft template. PDF/preview output
  is not native slide-object editing or creative approval.
- Animation uses engine-owned motion and explicit rig/part data. Inspect source
  identity, joints, rest pose, and actual output before claiming a successful rig.
- Lyrics designs lettering and motion against the supplied timing.
- Studio uses the project's HyperFrames composition. Runtime/tool failures stay
  visible rather than being hidden behind a substitute custom pipeline.

[MAGIC-STUDIO.md](../../MAGIC-STUDIO.md) consolidates intake, artwork, animation,
lyrics, video handoff, and project-agent guidance. Reusable Python tools,
pipeline YAML, stage directors, and skills remain in place; see
[project context](../../PROJECT_CONTEXT.md).

## Collaboration and verification

Coordinate owned files, preserve user edits/deletions, and use guarded shared
commands for UI/CLI/MCP parity. Check pending/dirty state before restarting a
shared app. Inspect the actual native window and saved output after a visible
change. Follow [CONTRIBUTING](../../CONTRIBUTING.md); behavioral tests require
Jesse's observed-UI approval and must not be inferred from historical checklists.

Detailed earlier implementation notes and project-specific observations remain
in Git history, rather than competing with these current instructions.
