# Magic Studio (`AGENTS.md`)

**Magic Studio** (`magic`) is the ArtCraft music-video and storybook animation studio. Artists take a song, read-only artist lyrics/actions, mixed-media character cutouts, scenes, and storyboards (`DesignCraft`), choreograph lyric lettering and motion overlays (`EffectCraft`), animate rigged characters, and assemble/render the final film (`FilmCraft`).

## Core Principles
1. **Prove in the Live App, Project Files & Final Video First**: Keep `magic` open while working. Do **not** lead with synthetic TDD unit tests—prove every feature first through the live UI (`--bridge`), the saved project files on disk (`.magic`, `design/words.json`, `lettering/lyrics.ecproj`, `storyboard/storyboard.designcraft`, `film/*.fcproj`), and the rendered video output.
2. **Use ArtCraft Crates & `--control` / `<app>-cli` (`CraftHub`)**: Link shared Craft libraries (`storytold/craft-libs`, `storytold/craft-ai-integrations`, `filmcraft-time`, etc.) and drive live/headless Craft apps via [`crates/engine/src/craft_hub/mod.rs`](crates/engine/src/craft_hub/mod.rs) (`--control` loopback TCP when open live, or installed `<app>-cli` on `PATH`). Never generate or splice HTML/CSS/JS inside Rust engine modules.
3. **One Engine, Thin Shell, Credentials in Rust**: `crates/engine` (`Session::execute`) is the single command dispatch shared by the desktop UI (`apps/magic`) and CLI (`apps/magic-cli`). Remote API credentials stay in Rust (`image_generation.rs` via `ureq`, `skills_registry.rs` via `curl`); webview `http:default` in `capabilities/default.json` is strictly loopback-only (`http://127.0.0.1:*`, `http://localhost:*`) with explicit `deny` on `https://api.openai.com/*` and `https://api.storyteller.ai/*`. User preferences live only in `config.toml` (`craft_settings`). Artist words and actions are strictly read-only. Never vendor external egui/wiki audit docs into Tauri React—link them instead.
4. **Paths & Authority Docs**: The desktop app lives at `apps/magic/` (`apps/magic/src/`, `apps/magic/ui/`, `apps/magic/capabilities/default.json`, `apps/magic/tauri.conf.json`). Ecosystem references: `storytold/artcraft`, `storytold/artcraft-services`, `storytold/craft-wiki`, `storytold/craft-libs`.

## Launching & Controlling the Live App (ArtCraft `--control` / `--bridge` Pattern)
Magic and every ArtCraft app share the same live-bridge contract (`<app> --control <PORT>` driven by `<app>-cli --bridge 127.0.0.1:<PORT>` or `CraftHub`, documented in [`docs/control-protocol.md`](docs/control-protocol.md) and [`crates/engine/src/craft_hub/mod.rs`](crates/engine/src/craft_hub/mod.rs)):
- Default `--control` ports: `filmcraft` `9875`, `magic` `9876`, `designcraft` `9877`, `effectcraft` `9878`, `photocraft` `9879`, `vectorcraft` `9880`, `texturecraft` `9882`.

Both `magic` and `magic-cli` share the same 64-hex `MAGIC_CONTROL_TOKEN` (`export MAGIC_CONTROL_TOKEN=$(openssl rand -hex 32)`):
```bash
# 1. Build frontend + desktop binary (when code changes)
(cd apps/magic && bun run build) && cargo build -p magic -p magic-cli --bins

# 2. Launch the live desktop app on loopback control port 9876
target/debug/magic --control 9876 --view project --project "/path/to/Project.magic"

# 3. Inspect & drive the live UI via --bridge (never combine --project with --bridge)
target/debug/magic-cli --bridge 127.0.0.1:9876 ui inspect
target/debug/magic-cli --bridge 127.0.0.1:9876 ui elements '{"prefix":"words"}'
target/debug/magic-cli --bridge 127.0.0.1:9876 ui click <element-id>
target/debug/magic-cli --bridge 127.0.0.1:9876 ui set "id=<element-id>" "value=<val>"
target/debug/magic-cli --bridge 127.0.0.1:9876 ui set "view=motion"
target/debug/magic-cli --bridge 127.0.0.1:9876 ui screenshot /tmp/magic-live.png

# 4. Run engine commands in the live session or headlessly
target/debug/magic-cli --bridge 127.0.0.1:9876 exec project.open "path=/path/to/Project.magic"
target/debug/magic-cli --bridge 127.0.0.1:9876 exec project.save
target/debug/magic-cli --project "/path/to/Project.magic" inspect project
target/debug/magic-cli commands
```
## References
- Desktop & plugin overview: [`apps/magic/README.md`](apps/magic/README.md) · Command & bridge contract: [`apps/magic/MAGIC.md`](apps/magic/MAGIC.md) · [`docs/control-protocol.md`](docs/control-protocol.md) · [`crates/engine/src/craft_hub/mod.rs`](crates/engine/src/craft_hub/mod.rs)
- Layer architecture & policy: [`docs/architecture.md`](docs/architecture.md) · [`xtask/src/policy.rs`](xtask/src/policy.rs)

- Layer architecture & policy: [`docs/architecture.md`](docs/architecture.md) · [`xtask/src/policy.rs`](xtask/src/policy.rs)
