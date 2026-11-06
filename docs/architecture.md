# Architecture

> **Last updated:** 2026-11-05 · **Model:** the Storytold craft apps (VectorCraft, DesignCraft, FilmCraft, EffectCraft)

How Magic is built. Magic is a craft app: it follows the same architecture as the other Storytold craft
apps, so reading any of them explains this one. When a change does not fit this file, stop and ask; do not
build around it.

## Shape

A Cargo workspace: library crates under `crates/`, apps under `apps/`, and `xtask`. **Pure Rust, no
handwritten JS/TS, no Tauri, no npm in the product.** The same engine runs the desktop app, the browser
build (egui on WASM), the headless CLI and the MCP server.

Reference designs (`references/*.tsx`, retired web UIs in Git history) are **design specs only**. They are
never built, bundled or shipped.

## Layers

`cargo xtask layers` (part of `cargo xtask ci` and the pre-commit hook) enforces that a crate depends only on
lower layers, and that nothing below L6 depends on a UI toolkit (`egui`, `eframe`, `winit`, `rfd`). The table
lives in `xtask/src/policy.rs`.

| Layer | Crates | Role |
|---|---|---|
| L0–L4 | craft engines and shared craft crates (`designcraft-*`, `effectcraft-*`, `filmcraft-*`, `photocraft-*`, `vectorcraft-*`, `texturecraft-*`, `craft-*`, `craft-ai-*`) | maintained dependencies, pinned to immutable revisions; never edited here |
| L5 | `engine` (package `magic-engine`) | sessions, history, the command registry, project files, the panic guard. Domain modules: `words` (lyrics design), `storyboard`, `character`, `film`, `song`… |
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
and MCP all reach the same registry, so anything a user can do an agent can do. Option lists the UI offers
(kinetic acts, layouts, letterings, overlays, strokes…) come from engine commands (e.g. `words.read` →
`kinetic_library`), not from constants in the UI.

## Views and craft canvases

The desktop views are egui, in `crates/ui-egui`, one module per view (Projects, Storyboard, Lyrics, Animation,
Studio). A view that needs another craft app's canvas embeds that app's egui crate at L6, on the same egui fork
revision: the Animation canvas and timeline come from `effectcraft-ui-egui` (Composition viewer, Timeline),
Studio from FilmCraft. Magic does not reimplement a craft app's canvas.

**Lyrics designer:** lyric text and timings come from the artist's sheet and the timing reference. The
designer reads and places them; it never edits them.

## Agent control

- **Control channel:** `magic --control PORT`: JSON lines on a loopback TCP port, with UI state, element
  listing, input injection and screenshots (`ui.*` commands).
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
| engine package is `magic` and `apps/magic` holds package `magic-gui`; the craft rule is `magic-engine` and `magic` | #91 |
| `xtask/src/policy.rs` is FilmCraft's layer table, not Magic's | #91 |
| craft git dependencies without `rev` (8 craft-libs revisions in `Cargo.lock`) | #91 |
| egui views are stand-ins without the accepted design; Animation lacks the EffectCraft canvas | #102 |
| engine has no document model, undo, link watch or guard; commands write files directly and views keep their own copies | #103 |
