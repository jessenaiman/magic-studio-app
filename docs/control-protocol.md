# Control protocol & MCP

Structure and shared rules follow FilmCraft's
[`docs/control-protocol.md`](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md);
craft-libs [`docs/ui-docking.md`](https://github.com/storytold/craft-libs/blob/main/docs/ui-docking.md) defines docking.
This file covers what is Magic's own: its views, its pipeline commands, and the Craft apps it embeds.

## Desktop control channel
`magic --control 9876` (or `MAGIC_CONTROL_PORT`) listens on `127.0.0.1:<port>` (loopback only).
The listener requires `MAGIC_CONTROL_TOKEN`: 64 random hex characters shared with authorized
clients through their inherited environment. Generate a fresh token for each desktop session, then
start the desktop and bridge from that environment:

```sh
export MAGIC_CONTROL_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
magic --control 9876 &
magic-cli mcp --bridge 127.0.0.1:9876
```

The CLI / MCP bridge reads this variable automatically. Keep it private: don't put the value in
command-line arguments, logs, checked-in MCP configuration, or shared files. Without a valid value,
the desktop leaves the listener closed. Every raw JSON request must include the same top-level
`token` string; missing or incorrect credentials get an error and close the connection before any
request is dispatched. Loopback alone does not restrict clients to the desktop owner's OS account.

Socket reads and writes time out after two minutes without progress, releasing the connection slot.
Expiration closes the connection without an unsolicited reply; reconnect before the next request
(the CLI / MCP bridge does this automatically). Waiting for a UI reply has its separate 60-second
limit. A partial request that times out is not dispatched.

One JSON request per line → one JSON reply per line.

**Only requests are read.** Every line must be a JSON object with a string `method` (`id` and `params`
are optional; blank lines are skipped). Anything else gets one error reply
(`{"ok": false, "error": "… closing the connection"}`) and the server **closes the connection**, so
nothing sent after it on that connection runs. That covers text that isn't JSON, a JSON array or number,
an object without `method`, invalid UTF-8, and a line longer than 4 MiB. An HTTP request (for example a
web page's cross-origin `fetch` to `127.0.0.1:<port>`) therefore can't smuggle a command in its body:
its request line is rejected first. At most 16 connections are served at once; further ones get an error
line and are closed. Clients that get an error reply should reconnect. Transport:
`apps/magic/src/control_server.rs`.

```json
{"id": 1, "token": "<MAGIC_CONTROL_TOKEN>", "method": "engine.execute", "params": {"command": "workshop.status", "params": {}}}
{"id": 1, "ok": true, "result": {"stage": "storyboard", "next": "storyboard.new"}}
```

Methods (handlers in `crates/ui-egui/src/control.rs`):

| Method | Params | |
|---|---|---|
| `engine.execute` / `ui.menu.invoke` | `{command, params}` | any engine or UI command id |
| `engine.commands` / `ui.menu.list` | – | command registry / menu tree |
| `ui.inspect` | – | UI state (view, open project, selected page / character / clip, open drawer, dock, playback, busy command, status, last error…) |
| `ui.docking` | – | the panel layout (craft-ui `Layout`), the focused and maximized panel, and `hitRects`: each docking gesture source drawn last frame (`{hit, rect}`) |
| `ui.elements` | `{prefix?}` | every on-screen interactive element: id, label, rect |
| `ui.set` | `{view, drawer, focused, theme, …}` | change UI state; an unknown field is an error, checked before anything changes |
| `ui.panel.show` / `ui.panel.close` | `{panel}` | |
| `ui.click` / `ui.move` | `{id}` or `{x,y}`, `button`, `count`, `modifiers` | synthetic pointer input; click `count` is an integer 0–10 (default 1); `ui.move` ignores it |
| `ui.drag` | `{from, to, steps, modifiers}` | press–move–release; `steps` is an integer 0–1024 (default 12; 0 or 1 uses 2 steps) |
| `ui.scroll` | `{id|x,y, dx, dy, modifiers}` | wheel / trackpad |
| `ui.key` / `ui.type` | `{key}` (`Cmd+K`, `Space`…; off macOS `Ctrl+K` is the same key) / `{text}` | keyboard |
| `ui.playback` | `{action: play|stop|toggle, speed?}` | the Studio and Animation transports |
| `ui.screenshot` | `{path?, panel?}` | PNG of the window or one panel; fails after 10 s when no frame is presented (window hidden, display asleep) |
| `ui.resize`, `app.quit` | `{width,height}` / `{}` | resize or close the native window; `app.quit` quits without the save prompt, while the command `app.quit` asks first |
| `ui.focus` | `{}` / `{id}` | empty parameters activate the native window; a widget ID requests egui focus without OS activation |

For `ui.click` and `ui.drag`, omitted or null counts/steps use the defaults. Out-of-range or
non-integer values return `{"ok": false, "error": "…"}` before queuing any pointer input.
These limits also apply to the MCP bridge's `ui_click` and `ui_drag` tools.

**Selection or explicit targets.** A command that acts on the selection is "not available right now" when
nothing is selected. When `params` name the targets under a key the command documents (`page`,
`character`, `part`, `line`, `clip`), enablement is checked against those instead and the selection is
left as it is; the command's other conditions still apply. `engine.commands` and the menus always show
the selection's enablement.

**Focus.** Driving the app never steals the user's keyboard. Started with `--control`, the app opens
without activating itself. UI requests and screenshots that need a rendered frame bring the window
forward without making it key (macOS `orderFrontRegardless`), so the user's typing keeps going to
the app they are in. Only an explicit `ui.focus {}` activates Magic. Injected keys work on
focused widgets even when the native window is in the background.

`ui.focus {id}` targets an enabled, visible widget registered with a real egui response. It rejects
unknown IDs, non-widget regions and hidden tool aliases. The reply acknowledges the request;
`ui.inspect.keyboardFocus` reports the actual widget ID, matching element IDs, label and active
focus. This is separate from `ui.focused`, which identifies the active panel.

For unattended validation, start with `--control <port> --control-input-only`. This opt-in mode
accepts widget input through control requests and excludes native keyboard, pointer, file-drop
and menu commands. Window close/quit and screenshots remain available. Ordinary launches and
`--control` alone keep native input enabled.

Element ids are stable, e.g. `view.storyboard`, `view.animation`, `drawer.scribbles`,
`storyboard.page.<n>`, `lyrics.line.<n>`, `character.<id>.part.<part>`, `panel.tab.<Panel>`.

## Views and the Craft apps they embed

Magic is a pipeline orchestrator: each view is one stage, and every action in it is a registry
command (see [architecture.md](architecture.md) › The pipeline). `ui.set {view}` switches views.

| View (`view`) | Stage | Commands | Embedded Craft UI |
|---|---|---|---|
| `projects` | open / create `.magic` projects, song intake | `project.*`, `workshop.set_audio`, `audio.analyze`, `workshop.import_transcript`, `workshop.status` | – |
| `storyboard` | storyboard and panels | `storyboard.new`, `storyboard.cues`, `storyboard.lyrics`, `storyboard.run`, `storyboard.export`, `workshop.split_panels` | DesignCraft document (`storyboard.designcraft`) |
| `lyrics` | kinetic lyrics | `words.design`, `words.letter`, `words.apply`, `words.compose` | HyperFrames; the Kinetic Lyrics Designer window (`magic --lyrics`) |
| `animation` | character cut, rig, actions, shots | `character.mark`, `character.cut`, `character.rig`, `character.action`, `character.render`, `character.shot` | EffectCraft (`effectcraft-ui-egui`) |
| `studio` | assembly and FilmCraft handoff | `video.assemble`, `video.init`, `video.render`, `video.preview`, `video.snapshot`, `video.export`, `video.exec`, `video.film` | FilmCraft |

Inside an embedded view, the embedded app's element ids, `ui.set` fields and command ids apply as
documented in that app's own control protocol
([EffectCraft](https://github.com/storytold/effectcraft/blob/main/docs/control-protocol.md),
[FilmCraft](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md)), reached
through `engine.execute` with the app's prefix (`effect.exec`, `video.exec`, `photo.exec`,
`vector.exec`). Magic does not redefine them.

Drawers: `fidget` and `scribbles` (`ui.set {drawer}`); Scribbles commands are `scribbles.*`.

## MCP
`magic-cli mcp` serves MCP on stdio: headless (in-process session; `--project p.magic`)
or `--bridge 127.0.0.1:9876` to drive the running app. Tools: `command_list`, `command_run`,
`command_batch`, `doc_inspect`, `render_preview`, `project_inspect`, `ui_inspect`, `ui_elements`,
`ui_click`, `ui_drag`, `ui_key`, `ui_type`, `ui_screenshot`; resources `magic://document`
and `magic://commands`.

## Panel docking commands

The panels use craft-ui's shared docking with its split-tree settings. Its control commands
(`window.panel.move`, `.float`, `.dock`, `.activate`, `.close`, `.closeGroup`, `.closeOthers`,
`.minimize`, `.layout`; see craft-libs `docs/ui-docking.md`) route through `ui.menu.invoke` and
`engine.execute`. Invalid source/target identities and non-finite or out-of-range geometry return an
error with the layout unchanged. `ui.docking` lists every docking gesture source. Named workspace saves
include floating windows.
