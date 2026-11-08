# Control protocol and MCP

Magic follows the Craft split: one Rust engine/session, a thin desktop shell,
and a CLI that runs either headlessly or against the live desktop. Tauri replaces
egui in the active shell. The references are FilmCraft's
[control protocol](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md),
[control transport](https://github.com/storytold/filmcraft/blob/main/apps/filmcraft/src/control_server.rs),
[CLI `--bridge` client](https://github.com/storytold/filmcraft/blob/main/apps/filmcraft-cli/src/main.rs),
and [rmcp server](https://github.com/storytold/filmcraft/blob/main/crates/automation/src/server.rs),
alongside EffectCraft's [MCP guide](https://github.com/storytold/effectcraft/blob/main/docs/mcp.md).

## Launch and connect

[The desktop guide](../apps/magic/README.md#run-the-desktop) contains
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

Method names, parameters and error rules follow FilmCraft's
[control protocol](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md).
A parameter a method does not name is an error (`unknown parameter ...`), never ignored.

| Method | Params | Behavior |
| --- | --- | --- |
| `engine.execute` | `{command, params}` | Dispatches to the same session as Tauri IPC. |
| `engine.commands` | `{filter?}` | Returns registry descriptions. |
| `ui.inspect` / `ui.state` | – | Actual frontend view, requested view, project, busy state, alerts, preserved-workspace feedback, `viewport`, `focus` and `cspViolations` (Content-Security-Policy blocks seen in the page). |
| `ui.drafts` | `{}` | Returns `{dirty, drafts, saving}` across mounted editor workspaces, including hidden workspaces. Unavailable workspace state is reported as a draft. |
| `ui.elements` | `{prefix?}` or `{unidentified: true}` | DOM IDs, labels, values, enabled state and window-coordinate bounds across the active view and its same-origin workspace frames. Audio/video elements also include a read-only `media` snapshot: current time, finite duration (otherwise null), paused/ended/seeking state, readiness/network codes, playback rate, volume/mute, buffered/seekable ranges, video dimensions, and native media error. Compare successive snapshots to verify playback advances; a successful Play click alone does not establish playback. `unidentified: true` instead lists visible interactive elements that have no ID (the Craft rule is that every interactive widget has one), with label, class and nearest identified parent. |
| `ui.click` | `{id}` or `{x,y}`, `button?` (`left`/`middle`/`right`), `count?` 1–10, `modifiers?` | pointer-down, pointer-up and click (or `auxclick` / `contextmenu`) at the element's center or the point; `count: 2` also sends `dblclick`. |
| `ui.move` | `{id}` or `{x,y}`, `modifiers?` | pointer and mouse move events. |
| `ui.drag` | `{from, to, steps?, modifiers?}` | Press at `from`, move through `steps` points (integer 0–1024, default 12), release at `to`. Each end is `{id}` or `{x,y}`. |
| `ui.scroll` | `{id}` or `{x,y}`, `dx?`, `dy?` | Wheel event, then scrolls the nearest scrollable ancestor if the page did not handle it. |
| `ui.key` | `{key, modifiers?}` | `Enter`, `Tab`, `Escape`, `ArrowDown`, `a`, or a chord such as `Ctrl+K` (`Cmd` is `Ctrl` here). Sends keydown and keyup to the focused element and, unless the page cancelled the event, performs the default for printable keys, Backspace, Delete, Tab, Enter, Space, and arrows on range and select controls. |
| `ui.type` / `ui.text` | `{text, id?}` | Types into the focused text field (or focuses `id` first) as one input event; at most 10,000 characters. |
| `ui.set` / `ui.set_view` | `{view}`; `{id, value}` for text, number and select controls; `{id, checked}` for checkboxes and radios | Navigation or a form change through the control's own events. |
| `ui.focus` | `{}` or `{id}` | Empty params activate the native window; an ID focuses that DOM element. |
| `ui.resize` | `{width, height}` | Logical window size, 320×240 up to 7680×4320. Returns the size actually reached; fails if the window manager keeps another size (tiled and maximized windows cannot be resized). |
| `ui.screenshot` | `{path?}` | PNG of the window as the webview presents it. `path` must be absolute and end in `.png`; without it a file is written to the temp folder. Returns `{path, width, height, bytes}`. Linux (WebKitGTK) only; fails after 10 s if no frame is presented (hidden window, sleeping display). |
| `ui.menu.invoke` / `ui.menu.list` | – | Aliases for `engine.execute` / `engine.commands`: Tauri has no native menu bar. |
| `app.quit` / `app.restart` | `{}` | Quits or restarts after replying when engine documents and project are saved, editor drafts are clear, and no operation is busy. Returns an error if state cannot be verified or work is unsaved or pending; save or wait before retrying. |
| `app.hide` | `{}` | Hides the main window. |

Pointer, key and text input are **DOM events dispatched by the page**, not operating-system
input. They reach every handler and the defaults listed above, but they cannot trigger CSS
`:hover` or native drag-and-drop and do not reach cross-origin frames (the HyperFrames preview).
Coordinates are window coordinates, the same space as the bounds `ui.elements` returns.

View names: `home`, `project` (alias `projects`), `storyboard`, `design` (alias
`lyrics`), `motion`, `gallery`, `animation`, `studio`. Lyrics and Motion share one mounted authoring session so view changes retain drafts. Gallery is a project-independent effect-example catalogue and is available without an open project. Without a project the rendered view is Home;
`requestedView` retains the requested navigation state. Other `ui.*` methods return
explicit unsupported-operation errors. Playback and panel control still require actual
implementations during restoration.

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
- `ui_click {id | x,y, button?, count?, modifiers?}`
- `ui_drag {from, to, steps?, modifiers?}`
- `ui_scroll {id | x,y, dx?, dy?}`
- `ui_key {key}`
- `ui_type {text, id?}`
- `ui_screenshot {path?}`
- `ui_control {method, params?}` (any other `ui.*` method, e.g. `ui.set`, `ui.resize`)

The UI tools require the live bridge. Unsupported UI operations are errors, not
headless substitutes. `project.identity {}` reads the current session's project;
an explicit `path` can inspect another project's canonical identity.

## Registering the MCP servers

The repository's [`.mcp.json`](../.mcp.json) registers one server, `magic`, bridged to
`127.0.0.1:9876`, for any MCP client that reads it (Claude Code and others). It runs
`target/release/magic-cli`; build it first (`cargo build --release -p magic-cli`) or
install it with `cargo install --locked --path apps/magic-cli`. Other clients take the same command and
arguments with an absolute path, for example in Codex's `config.toml`:

```toml
[mcp_servers.magic]
command = "/absolute/path/to/magic-cli"
args = ["mcp", "--bridge", "127.0.0.1:9876"]
```

The bridge needs the desktop's `MAGIC_CONTROL_TOKEN` in the client's environment. Never write the token
into `.mcp.json`, a client config, or a command line. Without it, the bridge server reports the missing
token.

## Removed legacy paths

There is one Tauri configuration (`apps/magic/tauri.conf.json`) and one
startup parser (Clap). The CLI-plugin parser and the duplicate config under
`apps/magic` are removed. The standalone browser HTTP server, Unix-socket
auto-discovery, `--follow-window`, `mcp --connect`, old `magic_*` MCP tool aliases,
and duplicate CLI socket client are removed. CLI and MCP share the `Remote`
client. Update MCP client configurations to the explicit bridge command above;
existing processes must be restarted after rebuilding.

The authoring UI sources are loaded by the Tauri shell; they are not a second
Tauri launch configuration. UI restoration has not been completed by this change.
