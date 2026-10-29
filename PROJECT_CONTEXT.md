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

Magic Studio retains the Python tools and declarative pipeline system:

```text
Agent reads pipeline YAML → reads the stage director skill → calls tools
→ reviews artifacts → saves a checkpoint → requests configured human approval
```

Python supplies capabilities and persistence utilities; the agent makes creative
choices and follows stage/approval instructions. This toolkit can be reused
without treating every app interaction as a complete pipeline run.

| Layer | Source and purpose |
|---|---|
| Capabilities | `tools/tool_registry.py`, `tools/base_tool.py`, `tools/` |
| Production process | `pipeline_defs/*.yaml`, `skills/pipelines/` |
| Production guidance | `skills/INDEX.md`, `skills/core/`, `skills/creative/`, `skills/meta/` |
| Checkpoints and budgets | `lib/checkpoint.py`, `tools/cost_tracker.py` |
| Artifact contracts | `schemas/artifacts/`, `schemas/checkpoints/`, `schemas/pipelines/` |
| Visual playbooks | `styles/*.yaml`, `schemas/styles/playbook.schema.json` |
| Technology knowledge | Relevant `.agents/skills/`; preserve third-party content and notices |
| Composition adapters | `tools/video/video_compose.py`, `remotion-composer/`, HyperFrames adapters |
| Pipeline board | `backlot/` reads pipeline-produced artifacts/checkpoints |

The pipeline registry and manifest are the authority for tools, stages, required
artifacts, and approval gates. Installed providers are discovered, not inferred.
Pipeline composition can use Remotion, HyperFrames, or FFmpeg as approved; that
choice does not add another target to the desktop Studio.

## Extending the toolkit

- New tools inherit `BaseTool`, declare real capability/provider contracts, and
  enter registry discovery. Use selector-plus-provider routing where appropriate.
- New pipelines need a valid `pipeline_defs/` manifest and referenced director
  skills. Keep review focus, success criteria, canonical outputs, and gates clear.
- Keep canonical artifacts schema-valid. Preserve checkpoint history and append
  changed production decisions instead of silently rewriting them.
- Propose native acceptance expectations first; encode behavior only after Jesse
  observes and explicitly approves the workflow.

Read [AGENT_GUIDE.md](AGENT_GUIDE.md) for routing and production governance and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the pipeline internals.
