---
name: magic
description: Operate Magic Studio with magic-cli and its MCP server. Launch the graphical application with magic. Discover Magic commands from its shared application registry.
---

# Magic Studio

The entry-point contract is intentional:

- `magic` opens the native graphical application. It has no REPL or install banner.
- `magic-cli` prints command help; `magic-cli --help` describes terminal usage.
- `magic-cli mcp` serves Magic operations over MCP stdio.

Do not restore the retired `mc`, REPL, hand-written MCP protocol, separate command tables, or UI → CLI → same-window dispatch. This refactor supersedes historical handoff documents. Do not reinstall ArtCraft to repair Magic. Other installed apps retain their own documented CLIs; Magic is not an aggregate Craft MCP server.

## Discover and execute

```bash
magic-cli commands project
magic-cli storyboard --help
magic-cli storyboard create --help
magic-cli project new path=/tmp/my-magic-project/Example.magic
magic-cli project open path=/tmp/my-magic-project/Example.magic
magic-cli project list-assets
magic-cli exec project.list_assets
magic-cli project save
```

The command tree is generated from the Rust application registry. Dots in IDs become subcommands; underscores become hyphens: `project.add_asset` → `magic-cli project add-asset`. Use `key=value` arguments, or `--params '{"key":"value"}'`. A command that also has children uses `--params` for its own parameters (for example `motion path --params '{...}'`). Numeric and boolean JSON values retain their types. Parameter descriptions are engine documentation, not default values to copy blindly. Engine commands enforce revisions, project ownership and domain validation.

`magic-cli run steps.jsonl` accepts `{ "id": "project.open", "params": {"path":"/absolute/Example.magic"} }` per line. Steps run in order through the same session routing; an error stops the batch, without rolling back earlier steps. Headless batches and each MCP connection retain one native engine session, including active-asset selection. The engine holds the selected project writer lease until close, explicit project switching, or transfer to a Studio window. Separate CLI invocations retain the project binding, not transient engine selection. Do not assume a cross-domain undo stack. Discover the relevant domain's undo command and receipt requirements.

## Project and window targeting

```bash
magic /absolute/Example.magic
magic-cli studio list
magic-cli studio open /absolute/Example.magic
magic-cli ui feedback
magic-cli ui look output=/tmp/magic-window.png
```

`project open path=...` explicitly selects a project. If that project is already open, operations use its window's private socket. Otherwise they run through the same native engine without a window and persist mutations. No routine bridge port or headless flag is needed.

Once pinned to a window, commands refuse a disconnected instance or changed project; they never silently choose another window or fall back to file writes. Use explicit `project open` or `studio open` to select again. `magic-cli --project /absolute/Example.magic <command>` explicitly selects before running. Unsaved/pending UI work can block switching or closing; save or resolve it in the UI. `project save` flushes pending storyboard drafts before native saving. Confirm saved files or visible results, not just JSON receipts.

Use `MAGIC_SESSION_FILE=/absolute/session.json` or `--session-file` for an independent caller binding. Otherwise the runtime binding is scoped per Herdr pane when present. `session status` inspects it; `session clear` clears only the binding. `--dry-run` suppresses mutations; explicit `--project` still selects context.

## MCP setup

```json
{
  "mcpServers": {
    "magicstudio": {
      "command": "magic-cli",
      "args": ["--project", "/absolute/Example.magic", "mcp"],
      "env": {"MAGIC_SESSION_FILE": "/absolute/magic-mcp-session.json"}
    }
  }
}
```

Omit `--project` to select later with `command_run` (`id: project.open`, `params: {path: ...}`). The client must inherit a PATH containing `magic-cli`; otherwise use its absolute installed path. Starting the server does not open a window. Tools:

- `command_list`: filterable discovery of registry IDs and parameter documentation.
- `command_run`: execute a Magic command by `id` and optional `params`; `ui.look` returns image content.
- `studio_open`: explicitly open/focus and bind a visible Magic window.

MCP cancellation does not interrupt native work already in progress. The session remains serialized until that work finishes; a later command cannot overtake it. A socket reply timeout is an unknown outcome, not proof of cancellation or failure. Inspect live feedback and saved results before retrying. Registered long-running commands have a one-hour reply timeout; other commands use 30 seconds.

The official Python MCP SDK owns initialization, protocol negotiation, transport and errors. SDK 1.30.0 supports `2024-11-05`, `2025-03-26`, `2025-06-18`, and `2025-11-25`; do not claim newer versions merely because a newer specification exists.

## Upstream references

Follow [CLI-Anything HARNESS](https://github.com/HKUDS/CLI-Anything/blob/main/cli-anything-plugin/HARNESS.md), with this project's explicit no-REPL entry points, and the [official MCP Python SDK](https://github.com/modelcontextprotocol/python-sdk/tree/v1.x) and [MCP architecture](https://modelcontextprotocol.io/docs/learn/architecture). Add no new tests without first presenting their cases for user approval. Preserve user-deleted files.

Native `--headless run` is a stop-on-error batch; the adapter uses internal `--headless serve` for persistent requests so a validation error does not discard session state. Native pipe reads and writes have the registry-based deadline too. A lost/timed-out native session is unavailable until explicit `project open` recovery; recovery never replays the failed request or replaces a still-running engine. Discovery remains available.

For read-only saved-project observation without changing a session or window, use
`magic-cli project observe path=/absolute/Project.magic` (no global `--project`). Its canonical
project, file hash, timestamp and saved fields are described in [MAGIC.md](../../../MAGIC.md).
Unsaved window state and external files are not observed; this is not an audit report.

For the saved-project change-token path and current limited coverage, use
`magic-cli app changes` without global `--project`; see [MAGIC.md](../../../MAGIC.md).
Each observer keeps its own token and never clears the producer's file. Unavailable notification
state is not evidence that nothing changed.

`magic-cli audit` returns Codacy's actual JSON source-analysis report. See
[MAGIC.md](../../../MAGIC.md) for setup, optional repository targeting and coverage semantics.
`magic-cli tui` launches the installed Backstage terminal app; the audit and Magic registry
operations work independently of the TUI.
