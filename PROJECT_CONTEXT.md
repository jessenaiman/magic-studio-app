# Magic Studio — project context

Magic Studio is Jesse Naiman's desktop creative-production app. The canonical
repository is [jessenaiman/magic-studio-app](https://github.com/jessenaiman/magic-studio-app).
Original authorship and licensing are recorded in
[SOURCE-ATTRIBUTION.md](SOURCE-ATTRIBUTION.md) and [LICENSE](LICENSE).

## Desktop application

The Tauri shell in `apps/magicstudio-shell/` embeds four creative areas:
Storyboard, Animation, Lyrics, and Studio. Studio targets HyperFrames. The
Rust engine owns project data, command validation, and motion evaluation.
The shell UI, `magic-cli`, and MCP use that shared command registry; there is no
second Python implementation of project persistence or UI command behavior.

| Contract | Source |
|---|---|
| Current app behavior and agent interaction | `apps/magicstudio-shell/agent.md` |
| Command IDs and typed parameters | `crates/engine/src/commands.rs` |
| CLI/MCP routing and ownership | `agent-harness/MAGIC.md` |
| Installed app and source installation | `agent-harness/cli_anything/magic/README.md` |
| Storyboard, animation, lyrics, video workflow | `MAGIC-STUDIO.md` |
| Native acceptance approval policy | `CONTRIBUTING.md` |

Project media belongs in the user's selected production root outside the source
checkout. Preserve supplied media, transcript wording/timing, and editable native
documents. A command receipt, draft, or preview is not a completed creative review.

## Backstage observer

`apps/backstage/` is the separate Rust/Ratatui observer for saved project
references, PR handoffs, worktrees, native audit artifacts, and settings.
It shows source identity and unavailable observations explicitly. It is not an
agent session controller or a production orchestrator.
See [Backstage](apps/backstage/README.md) for exact read boundaries and controls.

## Reusable production toolkit

Magic Studio retains the declarative pipeline system and its production scripts:

```text
Agent reads pipeline preset YAML → follows the Video Production department agent
→ runs the pipeline/ steps → reviews artifacts → requests configured human approval
```

The agent makes creative choices and follows the stage/approval entries. This
toolkit can be reused without treating every app interaction as a complete
pipeline run.

| Layer | Source and purpose |
|---|---|
| Department agents | `agents/` — one agent file per department (storyboard, animation, lyrics, video production, mayhem) |
| Production process | `pipeline_defs/` (data-only presets) |
| Production scripts | `pipeline/` — the engine's animation, layout, and rigging steps |
| Production guidance | `skills/INDEX.md`, `skills/core/`, `skills/creative/`, `skills/meta/` |
| Technology knowledge | Relevant `.agents/skills/`; preserve third-party content and notices |
| Composition adapters | `remotion-composer/`, HyperFrames adapters |

The preset is the authority for stages, required artifacts, and approval gates.
Installed providers and CLIs are discovered, not inferred. Pipeline composition
can use Remotion, HyperFrames, or FFmpeg as approved; that choice does not add
another target to the desktop Studio.

## Extending the toolkit

- New production steps are `pipeline/` scripts invoked through the engine's
  command registry; discover the contract, never invent an operation.
- New pipelines need a `pipeline_defs/` preset with clear stage review
  focus, success criteria, canonical outputs, and gates.
- Keep canonical artifacts in their documented shape. Append changed production
  decisions instead of silently rewriting them.
- Propose native acceptance expectations first; encode behavior only after Jesse
  observes and explicitly approves the workflow.

Read [AGENT_GUIDE.md](AGENT_GUIDE.md) for routing and production governance and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the pipeline internals.
