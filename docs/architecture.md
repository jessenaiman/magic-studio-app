# Architecture (honest draft)

> **Last reviewed:** 2026-11-06 · **Model:** Storytold craft apps (FilmCraft, EffectCraft) · **Status:** partially unproven — see Known deviations

Magic is a pipeline orchestrator, not a video editor. It combines Craft tools into a FilmCraft project. Engine is headless: every feature reachable without a window; egui UI, CLI, MCP and `--control` are clients.

Design principles:
1. **Engine-first.** All project changes via `Session::execute(id, params)`.
2. **Everything is a command.** 162 `CommandSpec`s in `crates/engine/src/commands.rs`.
3. **Rust server, TS client (proposed).** Rust owns documents/Craft/paid backends; TS-React (or egui) only renders + emits IDs. No JS.
4. **No second dispatch.** CLI/MCP/--control/API resolve same registry.
5. **Honest time:** currently `f64` seconds in lyrics/motion, not FilmCraft `Tick`. Drift risk at 29.97 — unproven.

## 1. Layers (`xtask/src/policy.rs`, enforced by `cargo xtask layers`)

```
L7  apps/magic-tauri (active desktop) · apps/magic (legacy egui) · apps/magic-cli · apps/magic-tui · xtask
L6  crates/ui-egui (magic-ui-egui) · crates/mcp (magic-mcp)
L5  crates/engine (magic-engine)
L0-L4 Craft backends (designcraft-*, effectcraft-*, filmcraft-*, photocraft-*,
    vectorcraft-*, texturecraft-*, craft-*, craft-ai-*) — pinned, never edited
```

| Crate | Layer | Purpose (actual) |
|---|---|---|
| `magic-engine` | L5 | `Session`, 162 commands in `cmd/<domain>/`, `session/execute.rs`, `undo.rs`, `guard.rs`, `workspace.rs` (drives Craft CLIs / in-process Sessions) |
| `magic-ui-egui` | L6 | egui views: `lib.rs` (Projects/Storyboard/Lyrics/Animation/Studio), `animation_effectcraft.rs`, `studio_filmcraft.rs`, `storyboard_workspace.rs`, `control.rs` |
| `magic-mcp` | L6 | MCP stdio server, headless or `--bridge` to running app |
| `magic-tauri` | L7 | Active desktop shell: React/Tauri, one managed engine session, authenticated `--control` listener |
| `magic` | L7 | Legacy egui desktop retained as restoration reference |
| `magic-cli` | L7 | Headless CLI: `commands/describe/exec/run/mcp/ui/storyboard`, `--bridge`, `--project` |
| `magic-tui` | L7 | Terminal UI |
| `xtask` | L7 | `ci` (fmt/clippy/layers/control-protocol), `layers`, `install` |

Rules: every crate has a layer; only downward edges; no `egui/eframe/winit/rfd/tauri` below L6. No `magic-web` (deleted `2c499f64`).

## 2. Data model (`.magic` JSON)

```
.magic (project_path in Session)
├─ song: SongMetadata, song_audio, video_target, video_project_path
├─ project_mode: None (song) | Some("animation")
├─ assets: map id→Asset, project_assets: Vec<ProjectAsset>
├─ docs: Vec<DocState> (lyric design, storyboard, cast, film)
└─ edits_since_save, project_saves (atomic write via write_project_file)
```
Files written only on save. `link_watch.rs` reloads external changes. Undo = `session/undo.rs` snapshots.

## 3. Command system (actual struct)

```rust
pub struct CommandSpec { // crates/engine/src/commands.rs:26
  pub id: &'static str, pub label: &'static str,
  pub menu: &'static [&'static str], pub shortcut: Option<&'static str>,
  pub params: &'static str,
  pub enabled: fn(&Session) -> Result<(), String>,
  pub run: fn(&mut Session, &Value) -> Result<Value>,
  pub journal: bool,
}
```
162 commands, domains: `project, workshop, storyboard, words, character, video, effect, photo, vector, scribbles, fidget, image, audio, song, ui, studio, mayhem, backstage…` Verified: `magic-cli commands`.

## 4. Views — honest state

| View | egui (`ui-egui`) | JS/TS (`apps/magic-cli/ui`, retired #93) | Status |
|---|---|---|---|
| Projects | `lib.rs` refresh | `projects.html` landing only | Stand-in |
| Storyboard | `storyboard_workspace.rs` | `storyboard-workspace.html` | Working spread, fields editor |
| Lyrics | `lyrics/` | `lyrics.html` + `words*.js` (3,585 lines) | JS ahead of egui; TS migration needed |
| Animation | `animation_effectcraft.rs` | `animation.js` (928) + Konva `character-stage.js` | Canvas exists in JS, missing in egui (#102) |
| Studio | `studio_filmcraft.rs` | `studio.html` toolbar | Toolbar only |

## 5. Control / MCP / API

- `magic-tauri --control 9876`: authenticated JSON-lines control over loopback TCP; shares the session used by Tauri IPC.
- `magic-cli --bridge 127.0.0.1:9876 mcp`: Craft-style `rmcp` tools; no local session or automatic fallback in bridge mode. Omit `--bridge` for explicit headless operation.
- The separate browser HTTP UI server and duplicate CLI socket implementation are removed.
- See [control-protocol.md](control-protocol.md) for implemented UI operations and the canonical launch instructions.

## 6. Known deviations (true)

- Tauri is the chosen active shell; the frontend remains in `apps/magic/ui`. Older `apps/magic-cli/ui` assets and egui views remain restoration references, not parallel Tauri configurations.
- Craft git deps without `rev` in `Cargo.lock` (#91).
- `target/analytics/{audit,lizard}.json` stale, references deleted `apps/src-tauri` — Mayhem reads it, nothing regenerates it.
- `f64` seconds, no `Tick` — no exact time guarantee.
- Tests: 17 lib tests, all mock/local (scribbles template resolution only). No test fires `image.generate` (OpenAI `ureq::post` / Codex `exec`) — by design, but unproven that paid path works.
