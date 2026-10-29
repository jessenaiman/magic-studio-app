# Architecture reading guide

Use this guide to find the implementation before proposing a change. Start with
[project context](../../PROJECT_CONTEXT.md) for the current app/toolkit boundary.

## 1. Follow a desktop command

```text
Embedded desktop UI ── Tauri dispatch ──┐
magic-cli / MCP ── native transport ────┤
                                      v
                        Session::execute → CommandSpec → domain handler
                                      v
                         selected .magic project and native artifacts
```

| Responsibility | Read next |
|---|---|
| Window, embedded UI, guarded native dispatch | [`apps/magicstudio-shell/src/main.rs`](../../apps/magicstudio-shell/src/main.rs), [`project_dispatch.rs`](../../apps/magicstudio-shell/src/project_dispatch.rs) |
| CLI/MCP project binding and transport | [Command architecture](../../agent-harness/MAGIC.md) |
| Command metadata, validation, handler selection | [`crates/engine/src/commands.rs`](../../crates/engine/src/commands.rs), [`lib.rs`](../../crates/engine/src/lib.rs) |
| In-memory project and asset state | [`session.rs`](../../crates/engine/src/session.rs) |
| Project lifecycle and writer ownership | [`project.rs`](../../crates/engine/src/project.rs) |
| Storyboard / animation / lyrics / video domains | Engine modules in the [Rust API reference](../RUSTDOC.md) |

The registry is the authority for command IDs and parameters. Frontends should
use guarded dispatch rather than inventing another project implementation.
Production commands may write files or call external tools; documentation
generation does not execute them.

## 2. Keep Backstage separate

Backstage is a Rust/Ratatui observer, not another engine frontend. Its
[`main.rs`](../../apps/backstage/src/main.rs) coordinates explicit observations
and terminal events. `project_data` reads saved references, `provider` bounds
approved observation processes, `handoff` interprets PR metadata, and `audit`
reads existing native reports. View modules render that evidence.

Read the [Backstage guide](../../apps/backstage/README.md) for exact controls,
process boundaries, and unknown/stale states. It does not resume agents, execute
PR-supplied commands, or run checkers when browsing audit artifacts.

## 3. Locate the production toolkit

The retained Python tools and YAML pipelines supply production capabilities and
persistence; agents follow stage-director skills and review/approval gates.
They are not a second native-app installer. See
[pipeline internals](../ARCHITECTURE.md) and the
[agent guide](../../AGENT_GUIDE.md).

## Diagrams and implementation records

- [`magic-studio-current.mmd`](../magic-studio-current.mmd) is an existing Mermaid
  source snapshot of shell/transport/engine/artifact relationships. Some labels
  retain earlier CLI spellings and UI groupings; use the maintained command
  architecture and setup guide for current syntax, not the diagram labels.
- [`image-to-editable-animation.mmd`](../image-to-editable-animation.mmd) maps a
  particular artwork workflow, not the complete application architecture.
- [`magic-cli-proposed.mmd`](../magic-cli-proposed.mmd) is a proposal, not proof
  that every depicted route is implemented.
- [Entry-point review](magic-entry-points-review.md) records current transport
  boundaries and points to maintained contracts. Older handoffs remain scoped
  to the revisions and evidence they name.

Rustdoc supplies clickable module and source relationships, not an automatically
verified dependency or call graph. Pair it with these responsibility maps when
discussing a change.
