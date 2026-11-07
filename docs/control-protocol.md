# Control protocol and MCP

Magic follows the Craft split: one Rust engine/session, a thin desktop shell,
and a CLI that runs either headlessly or against the live desktop. Tauri replaces
egui in the active shell. The reference is FilmCraft's
[control transport](https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/apps/filmcraft/src/control_server.rs)
and [rmcp server](https://github.com/storytold/filmcraft/blob/7322aad72491a294a06a26a63fb65a3fc30ac242/crates/automation/src/server.rs).

## Launch and connect

[The desktop guide](../apps/magic-tauri/README.md#run-the-tauri-desktop) contains
the exact Tauri CLI launch command. `--control PORT` (or `MAGIC_CONTROL_PORT`)
starts a loopback JSON-lines listener. `MAGIC_CONTROL_TOKEN` must be 64 hexadecimal
characters and must be inherited by both desktop and CLI/MCP processes. Missing
or invalid tokens and bind failures fail startup; tokens are never logged.

`magic-cli --bridge 127.0.0.1:9876 mcp` serves MCP on stdio and forwards calls to
that live session. `magic-cli mcp` uses a headless session. With `--bridge`, the
CLI never creates a competing local session; open projects through
`exec project.open path=/absolute/Project.magic`.

The control channel is newline-delimited JSON over TCP, not HTTP or WebSocket.
It accepts at most 16 simultaneous connections, with 4 MiB request limits and
120-second socket read/write timeouts. Malformed or unauthenticated requests close
the connection without executing. Requests before frontend readiness fail with a
retryable message. UI requests wait at most 10 seconds for a frontend response.

```json
{"id":1,"token":"<inherited token>","method":"engine.execute","params":{"command":"project.identity","params":{}}}
{"id":1,"ok":true,"result":{"project":"/absolute/Project.magic","identity_kind":"canonical path","ok":true,"project_id":"/absolute/Project.magic"}}
```

## Implemented desktop methods

| Method | Behavior |
| --- | --- |
| `engine.execute` | `{command, params}` dispatches to the same session as Tauri IPC. |
| `engine.commands` | Returns registry descriptions. |
| `ui.inspect` / `ui.state` | Returns actual frontend view, requested view, project, busy state, alerts, and preserved-workspace feedback. |
| `ui.elements` | `{prefix?}` returns DOM IDs, labels, values, enabled state and bounds across the active view and its same-origin workspace frames. |
| `ui.set` / `ui.set_view` | `{view}` requests navigation. `ui.set {id,value}` changes a supported form control. Read `ui.inspect` or `ui.elements` to observe the result. |
| `ui.click` | `{id}` clicks a visible, enabled DOM element. |
| `ui.focus` | `{id}` focuses a DOM element; `{}` focuses the native window. |
| `app.quit` | Exits the application after replying; no save prompt. Save first. |

View names: `home`, `project` (alias `projects`), `storyboard`, `design` (alias
`lyrics`), `animation`, `studio`. Without a project the rendered view is Home;
`requestedView` retains the requested navigation state. Other UI methods return
explicit unsupported-operation errors. Drag, keyboard injection, screenshots,
playback and panel control still require actual implementations during restoration.

Frontend commands use Tauri IPC directly. External mutations refresh the frontend
through one session-snapshot path. External reads do not reset navigation. Engine
work runs off the event loop and session access is serialized.

## MCP tools

Magic uses `rmcp`, as FilmCraft does, rather than a bespoke JSON-RPC server.

- `command_list {filter?, enabled_only?}`
- `command_run {id, params?}`
- `command_batch {steps: [{id, params?}], stop_on_error?}`
- `ui_inspect {}`
- `ui_elements {prefix?}`
- `ui_click {id}`
- `ui_control {method, params?}`

The UI tools require the live bridge. Unsupported UI operations are errors, not
headless substitutes. `project.identity {}` reads the current session's project;
an explicit `path` can inspect another project's canonical identity.

## Removed legacy paths

There is one Tauri configuration (`apps/magic-tauri/tauri.conf.json`) and one
startup parser (Clap). The CLI-plugin parser and the duplicate config under
`apps/magic` are removed. The standalone browser HTTP server, Unix-socket
auto-discovery, `--follow-window`, `mcp --connect`, old `magic_*` MCP tool aliases,
and duplicate CLI socket client are removed. CLI and MCP share the `Remote`
client. Update MCP client configurations to the explicit bridge command above;
existing processes must be restarted after rebuilding.

Old egui and UI sources remain restoration references; they are not a second
Tauri launch configuration. UI restoration has not been completed by this change.
