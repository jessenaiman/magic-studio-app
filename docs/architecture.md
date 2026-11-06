# Architecture

> **Last updated:** 2026-11-06 · **Model:** the Storytold craft apps (VectorCraft, DesignCraft, FilmCraft, EffectCraft)

How Magic is built. Magic is not a video editor. It is a pipeline orchestrator: an app that
combines ~10 Craft tools to end up with a FilmCraft video project. It reuses each Craft app
through the CLI and the command registry — one `Session::execute` path, no second dispatch
table — and its job is the pipeline that connects the tools from different apps into a single
film. When a change does not fit this file, stop and ask; do not build around it.

## Shape

A Cargo workspace: library crates under `crates/`, apps under `apps/`, and `xtask`. **Pure Rust, no
handwritten JS/TS, no Tauri, no npm in the product.** The same engine runs the desktop app, the browser
build (egui on WASM), the headless CLI and the MCP server.

Reference designs (`references/*.tsx`, retired web UIs in Git history) are **design specs only**. They are
never built, bundled or shipped.

## The pipeline

Magic's destination is a FilmCraft project; most of its architecture is the pipeline that feeds
that destination. The pipeline is pipeline-style: each stage produces an artifact the next stage
consumes, and `workshop.status` returns the current stage and next action. Every arrow below is a
CLI-visible command.

| Stage | Artifact produced | Command |
|---|---|---|
| Song intake | `hyperframes/transcript.json`, `transcript.srt`, `audio-analysis.json`, beats | `workshop.set_audio`, `audio.analyze` |
| Producer-supplied words | timed words + lines | `workshop.import_transcript` |
| Storyboard | `storyboard.designcraft` | `storyboard.new`, `storyboard.cues`, `storyboard.lyrics`, `storyboard.run`, `storyboard.export` |
| Panel extraction | split panels | `workshop.split_panels` |
| Character art | `assets/characters/<name>/parts/` from drawings | `scribbles.*`, `image.generate`, `project.add_asset`, `character.*` |
| Cut / rig / pose | animated clips/frames | `character.mark` → `character.cut` → `character.rig/action/render` |
| Kinetic lyrics | `lyrics-design.json`, lettering layer | `words.design`, `words.letter`, `words.apply`, `words.compose` |
| Assemble | `hyperframes/` video project, `exports/<song>.mp4` | `video.assemble`, `video.init/render/preview/snapshot/export` |
| Video handoff | native FilmCraft project, songs with timings | `video.exec` (raw interchange), `video.film` |

Typical chain: `project.new → audio.analyze → storyboard.new → scribbles.generate → words.design → video.assemble`.

The engine's `cmd/<domain>/` modules are the pipeline layers (`words/`, `storyboard/`, `character/`,
`film/`, `song/`, `project/`). Each command writes intermediate artifacts and records them in the session.

## Underlying Craft tools (transfer vehicle)

Magic treats each Craft app as a backend. It drives them through one command registry and through
`crates/engine/src/workspace.rs`, which builds exact CLI argument vectors, runs subprocesses, or
links library Sessions in-process. Real reuse — see `photo.exec`, `effect.exec`, `vector.exec`,
`video.exec`. Magic never edits Craft sources; it drives their engines.

| Job | Magic commands | Underlying app(s) | How it connects |
|---|---|---|---|
| Images | `photo.exec` | PhotoCraft | in-process `photocraft_engine::Session` |
| Vectors | `vector.exec` | VectorCraft | in-process `vectorcraft_engine::Session` |
| Motion / effects | `effect.exec` | EffectCraft | in-process `effectcraft_engine::Session` |
| Storyboard | `storyboard.run` (`storyboard.designcraft`) | DesignCraft | native `.designcraft` doc via linked crates |
| Video | `video.assemble/render/export`, `video.exec` | FilmCraft + HyperFrames | native render, CLI, or installed app `--control` |
| Text / director | `words.design`, `scribbles.*` | HyperFrames, Codex image model | command-driven media generation |
| Character animation | `character.*`, `character.shot`, Tripo breakout/rig | PhotoCraft + EffectCraft + Tripo | pipeline: mark (Scribbles) → cut (PhotoCraft) → rig (EffectCraft) → shot |

The desktop views in `crates/ui-egui` (Projects, Storyboard, Lyrics, Animation, Studio) embed
other Craft apps' egui crates at L6 on the same egui fork — Animation (`effectcraft-ui-egui`),
Studio (FilmCraft). Magic reuses those crates; it does not rebuild a timeline, compositor, or
canvas.

## Layers

`cargo xtask layers` (part of `cargo xtask ci` and the pre-commit hook) enforces that a crate depends only on
lower layers, and that nothing below L6 depends on a UI toolkit (`egui`, `eframe`, `winit`, `rfd`). The table
lives in `xtask/src/policy.rs`.

| Layer | Crates | Role |
|---|---|---|
| L0–L4 | Craft backends — PhotoCraft, EffectCraft, VectorCraft, DesignCraft, FilmCraft, HyperFrames, Codex (`designcraft-*`, `effectcraft-*`, `filmcraft-*`, `photocraft-*`, `vectorcraft-*`, `texturecraft-*`, `craft-*`, `craft-ai-*`) | maintained dependencies, pinned to immutable revisions; never edited here. Magic drives them through `Session::execute` and `workspace.rs`. |
| L5 | `engine` (package `magic-engine`) | pipeline orchestration: sessions, history, the command registry, project files, the panic guard. Domain modules: `words` (lyrics design), `storyboard`, `character`, `film`, `song`… |
| L6 | `ui-egui` (`magic-ui-egui`), `mcp` (`magic-mcp`) | the egui front end (views, panels, dialogs); the MCP server (JSON-RPC over stdio) |
| L7 | `apps/magic`, `apps/magic-cli`, `apps/magic-web`, `apps/magic-tui`, `xtask` | desktop app (eframe, wgpu; binary `magic`); CLI (binary `magic-cli`, including `magic-cli mcp`); browser runner (egui on WASM); terminal UI; build tooling |

Each app folder holds the package of the same name (`apps/magic` → package `magic`, `apps/magic-cli` →
`magic-cli`), and the engine is `magic-engine`, as in every craft app (`apps/vectorcraft` → `vectorcraft`,
`crates/engine` → `vectorcraft-engine`).

## Engine shape

The engine follows the craft engine layout (VectorCraft `crates/engine`):

| Module | Role |
|---|---|
| `session.rs`, `session/documents.rs`, `doc_state.rs` | the open project and its documents (lyric design, storyboard, cast, film) live in the session; views, CLI, MCP and the control channel all read and edit that one copy |
| `session/execute.rs`, `session/edit.rs` | one execution path: command → document edit → dirty → views redraw; files are written only by saving the document |
| `session/undo.rs`, `history.rs` | undo and redo for every journaled edit |
| `journal.rs`, `steps.rs` | the record of executed commands; batches whose steps reference earlier results |
| `link_watch.rs` | a file of the open project changed by another app or agent is reloaded, never silently overwritten |
| `guard.rs` | a panic inside a command rolls the document back; frontends never crash |
| `file_access.rs` | the folders automation may read and write |
| `cmd/<domain>/`, `command_sets.rs` | commands grouped by domain (`words`, `storyboard`, `character`, `film`, `song`, `project`), registered in one place |

A view never keeps its own copy of document state, and a command never writes a project file behind the
session's back.

## Everything is a command

Every user-visible action is a `CommandSpec` in the engine's command registry (id, label, menu path,
shortcut, params doc, `enabled`, `run`, journal flag). Views emit commands; menus, the control channel, the CLI
and MCP all reach the same registry, so anything a user can do an agent can do. There is no second dispatch
table: `magic-cli <domain> <verb>` resolves against the same registry the desktop UI, headless engine, and MCP
server use. Option lists the UI offers (kinetic acts, layouts, letterings, overlays, strokes…) come from
engine commands (e.g. `words.read` → `kinetic_library`), not from constants in the UI.

## Views and craft canvases

The desktop views are egui, in `crates/ui-egui`, one module per view (Projects, Storyboard, Lyrics, Animation,
Studio). A view that needs another craft app's canvas embeds that app's egui crate at L6, on the same egui fork
revision. Magic does not reimplement a craft app's canvas.

## Agent control

- **Control channel:** `magic --control PORT`: JSON lines on a loopback TCP port, with UI state, element
  listing, input injection and screenshots (`ui.*` commands). Budgets and read/write confinement
  (`--automation-read-root` / `--automation-write-root`) are in [control-protocol.md](control-protocol.md).
- **MCP:** `magic-cli mcp`, attached to a running app (`--connect`) or headless.
- Other craft apps are driven the same way (`<app> --control PORT`, `<app>-cli mcp`).

## Robustness

No panics in shipped code (workspace clippy lints); errors reach the status bar. Craft dependencies are pinned
to immutable revisions.

## Known deviations (to fix)

| Deviation | Issue |
|---|---|
| `apps/magic` builds an optional Tauri webview and runs `npm` for the Lyrics design (`lyrics-webview` feature) | #97 |
| `apps/magic-cli/ui` (retired web UI), `ui_server.rs` (`magic-cli ui`), and npm files in `apps/magic-cli` | #93 |
| craft git dependencies without `rev` (8 craft-libs revisions in `Cargo.lock`) | #91 |
| egui views are stand-ins without the accepted design; Animation lacks the EffectCraft canvas | #102 |
