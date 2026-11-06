# Control protocol

> **Last updated:** 2026-11-06 · **Change:** rewritten for Magic. The previous text was a Craft app's
> protocol (brush and symbol libraries, docking, synthetic input budgets) that Magic does not implement.

A running `magic` window can be inspected and driven by `magic-cli`, `magic-cli mcp` and agents over a
JSON-lines control channel. One request per line, one reply per line:

```json
{"id": 1, "method": "ui.inspect", "params": {}}
{"id": 1, "ok": true, "result": {...}}
{"id": 1, "ok": false, "error": "..."}
```

`command` is accepted as a synonym for `method`. A line that isn't JSON gets
`{"id": null, "ok": false, "error": "parse error: …"}` and the connection stays open.

## Transports

| Transport | When | Auth | Code |
|---|---|---|---|
| Unix socket `$XDG_RUNTIME_DIR/magic/<pid>-1.sock` | always, on Unix | none (user-only runtime dir) | [`spawn_unix_control_socket`](../apps/magic/src/main.rs#L171) |
| Loopback TCP `magic --control <port>` (or `--control 127.0.0.1:<port>`) | only with `--control` | optional `--control-token` / `MAGIC_CONTROL_TOKEN`; each request then carries `"token"`, compared in constant time | [`spawn_control_listener`](../apps/magic/src/main.rs#L132), [`handle_control_stream`](../apps/magic/src/main.rs#L62) |

Both feed one channel into the egui app. Requests are drained once per frame
([`poll_control`](../crates/ui-egui/src/lib.rs#L599)) and answered by
[`handle_control_request`](../crates/ui-egui/src/lib.rs#L636). If the window closes first, the reply is
`"window closed"`.

Clients:

- `magic-cli` uses `--bridge <port>` for TCP. Otherwise it connects to the newest socket in
  `$XDG_RUNTIME_DIR/magic/` ([`call_live_control`](../apps/magic-cli/src/main.rs#L522)).
- `magic-cli mcp` runs headless, or forwards to a `--control` window
  ([`crates/mcp/src/lib.rs`](../crates/mcp/src/lib.rs)).

## Methods

Window methods are handled by the UI. Every other method is an engine command and goes to
`Session::execute`, the same registry the CLI and MCP use
([`crates/engine/src/commands.rs`](../crates/engine/src/commands.rs)).

| Method | Params | Result |
|---|---|---|
| `ui.inspect` / `ui.state` | | `view`, `project`, `assistant_open`, `assistant_tab`, `selected_page`, `selected_character`, `song_title`, `song_audio_path`, `lyrics_sections_count`, `storyboard_pages_count`, `characters_count`, `busy`, `status`, `last_error`, `unconfigured`, `widgets` |
| `ui.feedback` | | engine `ui.feedback`, plus `view`, `unconfigured`, `widgets_count`, `song_audio_path`, `lyrics_sections_count` |
| `ui.unconfigured` | | `{view, unconfigured: [{id, reason, fix}]}`: what the open project still lacks and the command that fixes it (project, song audio, lyrics, storyboard pages, character rigs) |
| `ui.elements` / `ui.components` / `ui.controls` | | `widgets` from the automation registry ([`automation.rs`](../crates/ui-egui/src/automation.rs)) and the fixed `elements` list below |
| `ui.focus` | `{id}` | focuses a registered widget; `{ok, id}` |
| `ui.set_view` | `{view}` (or `{id}`) | switches view; error for an unknown name |
| `ui.click` | `{id}` | one of the `elements` below; error for anything else |
| `ui.set` | `{id, value}` | `view`, `template` (assistant template), `prompt` (assistant prompt); other ids are ignored |
| `app.quit` | | closes the window |
| any other id | that command's params | `Session::execute(method, params)`; the UI then refreshes from the result |

Views (`ui.set_view`, `ui.set {id: "view"}`, `magic --view`): `projects` (`home`),
`storyboard` (`project`), `lyrics` (`words`, `design`), `animation` (`character`),
`studio` (`film`, `video`).

`ui.click` elements: `nav-projects`, `nav-storyboard`, `nav-lyrics`, `nav-animation`, `nav-studio`,
`drawer-fidget`, `drawer-scribbles` (the short names `projects`, `fidget`, … also work).

`magic --lyrics` (feature `lyrics-webview`) opens the Tauri Kinetic Lyrics Designer instead of the
egui window ([`apps/magic/src/lyrics.rs`](../apps/magic/src/lyrics.rs)). That window has no control channel.

## Not in Magic yet

These are parts of the Craft control protocol (PhotoCraft, VectorCraft, EffectCraft) that Magic
doesn't have. Until they're added, don't document them here as Magic behavior.

- `engine.execute {command, params}` and `engine.commands` (commands with enablement). Magic takes the
  command id as the method instead.
- Strict request handling: rejecting non-object lines and closing the connection, unknown-parameter errors.
- Transport budgets: a connection cap, idle / partial-line / reply timeouts, and a 60 s
  "will not run" / "still running" reply.
- Synthetic input: `ui.pointer`, `ui.key`, `ui.text`, `ui.move`, `ui.drag`, `ui.wheel`, `ui.screenshot`.
- `ui.click` by coordinates, and `ui.set` validating its field list before changing anything.
- Confining file access (`--automation-read-root` / `--automation-write-root`).
- Background job replies (`{job, pending: true}`, progress, cancel).
- Menu (`ui.menu.list` / `ui.menu.invoke`) and panel docking methods.
- A `--control` channel for the Tauri lyrics window.
