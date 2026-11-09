# Magic Studio Desktop (`apps/magic`)

`apps/magic` is the Tauri v2 desktop application for Magic Studio (`magic`). It embeds a single managed `magic_engine::Session` in Rust, exposes an authenticated loopback `--control <PORT>` listener on `127.0.0.1` (`9876` by default), and renders the workspace views (`projects`, `storyboard`, `design` / Lyrics, `motion` / Animation, `studio` / FilmCraft Studio) in React/TypeScript (`apps/magic/ui/`).

## Run the Desktop {#run-the-desktop}

```sh
# 1. Build the frontend bundle and Rust binaries
(cd apps/magic && bun run build) && cargo build -p magic -p magic-cli --bins

# 2. Launch the desktop app with live --control on port 9876
export MAGIC_CONTROL_TOKEN="$(openssl rand -hex 32)"
target/debug/magic --control 9876 --view project --project "/absolute/path/to/Project.magic"

# 3. Inspect and drive the live desktop UI via magic-cli --bridge
target/debug/magic-cli --bridge 127.0.0.1:9876 ui inspect
target/debug/magic-cli --bridge 127.0.0.1:9876 ui set "view=storyboard"
target/debug/magic-cli --bridge 127.0.0.1:9876 ui screenshot /tmp/magic-live.png
```

## Direct Craft Rust Engines & `--control` Bridge (`CraftHub`)

`magic-engine` links the ArtCraft Rust crates directly (`filmcraft-engine`, `filmcraft-export`, `filmcraft-gpu`, `filmcraft-render`, `filmcraft-project`, `filmcraft-codecs`, `filmcraft-media`, `filmcraft-time`, `effectcraft-engine`, `craft-settings`, `craft-scribbles`, `craft-ai-registry`) and coordinates sibling Craft applications through [`crates/engine/src/craft_hub/mod.rs`](../../crates/engine/src/craft_hub/mod.rs):

| Craft Domain | Authoritative Document | In-Process Rust Crates | Live `--control` Port |
|---|---|---|---|
| **FilmCraft** | `film/<Song>.fcproj` | `filmcraft-engine`, `filmcraft-export`, `filmcraft-gpu`, `filmcraft-render`, `filmcraft-project`, `filmcraft-codecs`, `filmcraft-media`, `filmcraft-time` | `9875` |
| **Magic Studio** | `<Song>.magic` | `magic-engine` (`Session::execute`) | `9876` |
| **DesignCraft** | `storyboard.designcraft` | `designcraft` live `--control` / `designcraft-cli` | `9877` |
| **EffectCraft** | `lettering/lyrics.ecproj`, `animation/*.ecproj` | `effectcraft-engine` (`HeadlessContext`) | `9878` |
| **PhotoCraft** | `*.pcraft` | `photocraft` live `--control` / `photocraft-cli` | `9879` |
| **VectorCraft** | `*.vcraft`, `*.svg` | `vectorcraft` live `--control` / `vectorcraft-cli` | `9880` |
| **TextureCraft** | `*.tcraft` | `texturecraft` live `--control` / `texturecraft-cli` | `9882` |

## Tauri v2 Plugins & Security Capabilities

Configured in [`tauri.conf.json`](tauri.conf.json) and [`capabilities/default.json`](capabilities/default.json):

| Plugin / Capability | Purpose |
|---|---|
| `core:default` / `core:window` / `core:webview` | Native window lifecycle, asset protocol (`asset://localhost/`), and IPC |
| `dialog` (`tauri-plugin-dialog`) | Native open/save file dialogs |
| `fs` (`tauri-plugin-fs`) | Scoped project file reads/writes |
| `http` (`tauri-plugin-http`) | Strictly loopback-only (`http://127.0.0.1:*`, `http://localhost:*`); external API hosts explicitly denied in webview |
| `opener` (`tauri-plugin-opener`) | Opening exported media and external documentation links |
| `clipboard-manager` | Pasting reference images into the project workspace |
| `notification` | Export and render completion notifications |
