# Architecture

> **Last reviewed:** 2026-11-09 · **Model:** Storytold Craft apps (FilmCraft, EffectCraft, AniCraft, DesignCraft)

Magic Studio (`magic`) is the ArtCraft music-video and storybook animation studio. The engine is headless: every feature is reachable without a window; the Tauri/React desktop UI, CLI, MCP, and `--control` loopback TCP bridge are thin clients over `Session::execute`.

Design principles:
1. **Engine-first.** All project changes via `Session::execute(id, params)`.
2. **Everything is a command.** 165 `CommandSpec`s in `crates/engine/src/commands.rs`.
3. **One Engine, Thin Shell, Credentials in Rust.** Rust owns documents, Craft sessions, and remote API credentials; TypeScript/React (`apps/magic/ui/`) renders state and dispatches command IDs. Never generate or splice HTML/CSS/JS inside Rust engine modules.
4. **No second dispatch.** Desktop UI, CLI, MCP, and `--control` resolve the same command registry.
5. **Exact `Tick` timebase (`filmcraft-time::Tick`):** All timeline, lyric, motion, marker, and frame commands operate on `Tick` (`254_016_000_000` ticks/second), eliminating floating-point frame drift across 24, 25, 29.97, 30, and 60 fps sequences.
6. **Direct Craft Rust Crates in `magic-engine`:**
   - **FilmCraft (`filmcraft-engine`, `filmcraft-export`, `filmcraft-gpu`, `filmcraft-render`, `filmcraft-project`, `filmcraft-codecs`, `filmcraft-media`, `filmcraft-time`):** Master song timeline, track grouping & label colors (`Lavender`/`Mango`/`Iris`/`Caribbean`), Whisper word/line alignment, Range/FlashCue markers (`markers.addRange`, `markers.addFlashCue`), and timeline interchange (`file.exportInterchange`).
   - **EffectCraft (`effectcraft-engine`, `craft-ecproj-format`, `craft-doc`):** Short-form scene and kinetic lyric compositions (`lettering/lyrics.ecproj`), 3D per-character text animators, path warps, camera moves, Comp/Cue-Point markers (`markers.set` with `cuePoint`), and timeline round-trip (`file.importTimeline`, `file.exportTimeline`).
   - **AniCraft (`anicraft-doc`, `anicraft-geom`):** Pure Rust animation math (`Easing` presets, `as_bezier()`, `apply()`, `Affine` decompose/compose, `Property`, `Track`, `MotionTween`, and `Symbol` / `LoopMode` evaluation) in `crates/engine/src/animation_model.rs`, plus optional live cel/rig studio via `CraftTarget::Ani` (`anicraft` / `anicraft-cli`, port `9881`).

## 1. Layers (`xtask/src/policy.rs`, enforced by `cargo xtask layers`)

```
L7  apps/magic (desktop) · apps/magic-cli · apps/magic-tui · xtask
L6  crates/mcp (magic-mcp) · UI/windowing crates (*-ui-egui, craft-ui, craft-dialogs, craft-keymap, egui, eframe, winit, tauri)
L5  crates/engine (magic-engine)
L4  crates/scribbles (craft-scribbles)
L0-L3 Headless Craft backends & shared libraries (anicraft-doc, anicraft-geom, effectcraft-engine,
      filmcraft-*, craft-atomic, craft-audio, craft-doc, craft-docpath, craft-ecproj-format,
      craft-media, craft-settings, craft-update, craft-ai-*) — pinned, never edited here
```

| Crate | Layer | Purpose |
|---|---|---|
| `craft-scribbles` | L4 | Google Flow (`Moppet Maker`) app-window launcher, handoff asset ingestor (`handoff.json`, `character.png`, `parts-sheet.png`, `pose-sheet.png`), animated SVG builder, and video clip bridge |
| `magic-engine` | L5 | `Session`, 165 commands in `cmd/<domain>/`, `animation_model.rs`, `session/execute.rs`, `undo.rs`, `guard.rs`, `workspace.rs`, `craft_hub/mod.rs` (drives Craft apps in-process, via `--control`, or `<app>-cli`; `config.toml` via `craft-settings`) |
| `magic-mcp` | L6 | MCP stdio server (`rmcp`), headless or `--bridge` to running app |
| `magic` | L7 | Desktop shell: React/Tauri (`apps/magic/ui` frontend) + native `filmcraft-ui-egui` / `effectcraft-ui-egui` child studio windows, one managed engine session, authenticated `--control` listener |
| `magic-cli` | L7 | Headless CLI: `commands/describe/exec/run/mcp/ui/storyboard`, `--bridge`, `--project` |
| `magic-tui` | L7 | Terminal UI |
| `xtask` | L7 | `ci` (build groups, fmt, layers, clippy, UI build), `layers` |

Rules: every workspace crate has a layer; edges must strictly go down (`dl < l`); no UI or windowing crates (`egui`, `eframe`, `winit`, `rfd`, `tauri`, `*-ui-egui`, `craft-ui`, `craft-dialogs`) allowed below L6.

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

## 3. Command system (`crates/engine/src/commands.rs`)

```rust
pub struct CommandSpec {
  pub id: &'static str, pub label: &'static str,
  pub menu: &'static [&'static str], pub shortcut: Option<&'static str>,
  pub params: &'static str,
  pub enabled: fn(&Session) -> Result<(), String>,
  pub run: fn(&mut Session, &Value) -> Result<Value>,
  pub journal: bool,
}
```
165 commands across domains: `project, workshop, storyboard, words, character, animation, video, effect, photo, vector, scribbles, fidget, image, audio, song, ui, studio, mayhem, backstage…` Verified via `magic-cli commands`.

## 4. Views (`apps/magic/ui/views/`)

| View | React Component (`apps/magic/ui/views/`) | Backend / Craft Integration |
|---|---|---|
| Projects (`project`) | `ProjectsView.tsx` | `project.*`, `song.*`, `audio.*`, `workshop.*` |
| Storyboard (`storyboard`) | `StoryboardView.tsx` | `storyboard.*`, `DesignCraft` (`storyboard/storyboard.designcraft`) |
| Lyrics (`design`) & Motion (`motion`) | `LyricsView.tsx` + `LyricsInspector.tsx` | `words.*`, `words.motion.*`, `video.markers`, `video.timeline`, `EffectCraft` (`lettering/lyrics.ecproj`), `AniCraft` (`anicraft-doc` easing/bezier/tween math) |
| Animation (`animation`) | `AnimationView.tsx` | `character.*`, `animation.*`, `scribbles.*`, `EffectCraft` (`effectcraft-ui-egui` / `.ecproj`), `AniCraft` (`anicraft-doc` symbols & tweens) |
| Studio (`film`) | `StudioView.tsx` | `video.*`, `video.markers`, `FilmCraft` (`filmcraft-engine` / `filmcraft-ui-egui` / `.fcproj`) |

## 5. Control / MCP / API

- `magic --control 9876`: authenticated JSON-lines control over loopback TCP; shares the session used by Tauri IPC. Methods follow FilmCraft: `engine.*`, `ui.inspect/elements/click/move/drag/scroll/key/type/set/focus/resize/screenshot`, `app.quit`.
- `magic-cli --bridge 127.0.0.1:9876 mcp`: Craft-style `rmcp` tools; no local session or automatic fallback in bridge mode. Omit `--bridge` for explicit headless operation.
- The separate browser HTTP UI server and old Unix-socket transport have been removed; restored workspaces use the native Tauri adapter.
- See [control-protocol.md](control-protocol.md) for implemented UI operations and the canonical launch instructions.
