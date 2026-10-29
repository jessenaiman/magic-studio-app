---
name: magic
description: Operate installed Magic Studio with magic-cli or MCP. Load existing .magic projects, access storyboard pages, and use linked HyperFrames inside the desktop Studio view.
---

# Magic Studio

## What and Where

Use installed `magic-cli` to load existing `.magic` projects and read/edit their
storyboard. `magic` opens the desktop app; its **Studio** view embeds the linked
HyperFrames workspace. Storyboard, Animation, and Lyrics are its creative areas.

## Why

Continue the existing production through shared app commands and saved state.
This is production onboarding, not installation or a second studio setup.

## How

1. Start with authoritative help: `magic-cli --help`, then
   `magic-cli studio open --help`, `magic-cli project --help`, or
   `magic-cli storyboard --help`. Use `magic-cli commands FILTER` for registry
   IDs/parameters, then the chosen command's `--help` before editing.
2. Select the supplied or saved/configured project explicitly using the help's
   open example. Use configured media, project, and animation roots; report
   missing paths/runtime without searching the machine or migrating folders.
3. In the app, select **Studio** for the linked HyperFrames workspace; use its
   existing Studio switch if off. Do not create another studio, install Python,
   or start a model/agent merely to discover capabilities.
4. Save intentional edits and inspect actual saved/visible results. A lost reply
   has an unknown outcome; inspect before retrying. A disconnected pinned window
   requires explicit reselection. For MCP entry instructions: `magic-cli mcp --help`.

## Success Criteria

The intended project is selected, its storyboard content or missing state is
reported, and its linked Studio is visible or the actual blocker is stated.
A command receipt or prepared workspace is not a finished video.

Details: [routing](https://github.com/jessenaiman/magic-studio-app/blob/HEAD/agent-harness/MAGIC.md),
[CLI-Anything](https://github.com/HKUDS/CLI-Anything/blob/main/cli-anything-plugin/HARNESS.md),
[official MCP SDK](https://github.com/modelcontextprotocol/python-sdk/tree/v1.x).
