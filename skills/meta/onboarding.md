# Magic Studio onboarding

## What and Where

Start with the installed `magic-cli` and the user's existing `.magic` project.
The agent can load it, read/edit its storyboard, and open/focus the app.
The app's Studio view embeds the project's linked HyperFrames workspace.

## Why

Help the user continue their production without a source bootstrap or another
studio. If the request is already actionable, begin that work immediately.

## How

Read [the Magic Studio entry skill](../../.agents/skills/magic/SKILL.md).
Use its verified project-selection, storyboard, and Studio steps. Use configured
production roots and supplied/saved paths; report missing paths without a
machine-wide search. Do not migrate folders, install Python, or start models.

For an open-ended greeting, ask one useful question about the song, footage,
lyrics, or existing project. Preserve supplied media, timing, and artwork.
Only read [pipeline guidance](../../AGENT_GUIDE.md) and the
[runtime matrix](../core/hyperframes.md) when that production workflow is requested.

## Success Criteria

The user knows how to select their existing project, access its storyboard, and
reach the linked HyperFrames workspace in Studio, with missing state stated
honestly. A draft or prepared project is not creative approval or a finished video.
