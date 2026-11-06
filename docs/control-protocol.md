# Control protocol

> **Last updated:** 2026-11-05 · **Model:** the craft-app control protocol (VectorCraft
> `docs/control-protocol.md`) · **Transport:** `apps/magic/src/main.rs` (`spawn_control_listener`),
> **handler:** `crates/ui-egui/src/lib.rs` (`handle_control_request`)

`magic --control <port>` listens on `127.0.0.1:<port>`; `--control <host:port>` binds that address as
given, so use a loopback host. `--control-token <secret>` makes every request carry `"token"`; it is
compared in constant time and a mismatch gets `unauthorized control token`.

One JSON request per line, one reply per line:

```json
{"id": 1, "method": "ui.state", "params": {}, "token": "optional"}
{"id": 1, "ok": true, "result": {...}}
{"id": 1, "ok": false, "error": "..."}
```

`command` is accepted as a synonym for `method`. A line that is not JSON gets
`{"id": null, "ok": false, "error": "parse error: …"}` and the connection stays open. Each connection runs
on its own thread; requests are applied on the UI thread between frames, and a reply waits for that
frame. Closing the window answers pending requests with `window closed`.

## Methods

| Method | Params | Result |
|---|---|---|
| `ui.state` | — | `view`, `project`, `assistant_open`, `assistant_tab`, `selected_page`, `selected_character`, `busy`, `status`, `last_error` |
| `ui.elements` (also `ui.components`, `ui.controls`) | — | the clickable ids: `nav-projects`, `nav-storyboard`, `nav-lyrics`, `nav-animation`, `nav-studio`, `drawer-fidget`, `drawer-scribbles` |
| `ui.click` | `{"id": "<element id>"}` | switches the view or opens the drawer; an unknown id is an error |
| `ui.set` | `{"id": "view" \| "template" \| "prompt", "value": "…"}` | sets the view, or the assistant's template or prompt |
| `app.quit` | — | closes the window |
| any engine command id (`words.read`, `storyboard.set`, `edit.undo`…) | the command's params (`magic-cli describe <id>`) | the command's result |

Engine commands run through the window's own session (`Session::execute`,
`crates/engine/src/session/execute.rs`), the same registry and execution path the CLI and MCP use, so
an edit made over the control channel is the app's edit: its documents, its undo history, its status bar.

## Clients

- **MCP:** `magic-cli mcp --connect 127.0.0.1:<port> [--token <secret>]` relays MCP tool calls to the
  window ([mcp.md](mcp.md)).
- **CLI:** `magic-cli <domain> <verb>` and `magic-cli <domain.verb>` resolve against the same registry
  (`crates/engine/src/command_catalog.rs`); `magic-cli commands` lists it and `magic-cli describe <id>`
  shows one command's params.
- **Other craft apps** are driven the same way: `<app> --control <port>` and `<app>-cli mcp`.

## Gaps against the craft control protocol

| Craft apps have | Magic today |
|---|---|
| `ui.screenshot` of the live window | registered in the engine, but the window does not answer it: the request reaches the engine stub and fails with `requires a live Magic Studio window` |
| element ids for every widget, `ui.inspect` | a fixed list of seven navigation and drawer ids |
| synthetic pointer and keyboard input (`ui.drag`, `ui.key`, typing) | none |
| request validation that closes the connection on non-requests, line-length and connection limits, idle and reply timeouts | none: one thread per connection, no limits |
| refusing a non-loopback bind | `--control host:port` binds any address given |

These are tracked with the engine and UI work in #103 and #102.
