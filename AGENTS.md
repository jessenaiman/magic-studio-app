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
Craft App Libraries
===================

<a href="https://sonarcloud.io/project/overview?id=storytold_craft-libs"><img alt="Scanned by SonarCloud" src="https://img.shields.io/badge/Scanned_by-SonarCloud-4b9fd5?style=flat-square&logo=sonarqubecloud&logoColor=white"></a>

This repo contains libraries used by the various Craft Apps. It's a Rust monorepo workspace, and the libraries can be included by any Craft Apps that need them.

In the future this may grow to include non-Rust library code.

If any libraries grow beyond Craft App usage, eg. RAW format support, they may be promoted out and into their own libraries for the broader ecosystem to take advantage of.


## Layout

A Cargo workspace (edition 2024, resolver 3). Each library is a crate under `crates/<name>`, named
`craft-<name>`, with `version`, `edition`, `license` and `repository` inherited from the workspace
(`license.workspace = true`, and so on) and `[lints] workspace = true`. Register each crate in
`[workspace.dependencies]` in the root `Cargo.toml`.

Apps depend on a crate by git:


### Craft-Libs 

Read the README, not the codebase and when you do find 3 assumptions that the codebase overengineered and fix them using these libraries

```toml
craft-<name> = { git = "https://github.com/storytold/craft-libs", rev = "<commit>" }
```

## Libraries

- [`craft-platform`](crates/platform/README.md): optional native dialog lifecycle, Linux/BSD desktop appearance observation and browser download ownership; application policy stays in the apps.

- [`craft-ai-core`](crates/ai-core/README.md): the data model and contract for AI in the Craft Apps (connections, models, tools, requests, jobs, consent, history, route spec). Pure, no I/O, wasm-safe. Contract in [docs/ai-shared-layer.md](docs/ai-shared-layer.md).
- [`craft-pdf`](crates/pdf/README.md): independently optional PDF parsing, interpretation, rasterization and guarded authoring. Apps retain layout, fonts, editing and export policy; writer-only consumers do not enable the shared reader.

- [`craft-atomic`](crates/atomic/README.md): Crash-safe atomic file writing and replacement with same-directory staging, directory fsync, Apple `F_FULLFSYNC` fallback, transient lock backoff, and fault injection for the Craft suite.
- [`craft-cms`](crates/cms/README.md): pure-Rust ICC v2/v4 colour management: profile parsing
  and writing, transforms, rendering intents, black point compensation, soft proofing, gamut
  checks, 3D LUTs and built-in profiles (sRGB, Display P3, synthetic coated CMYK and others).
- [`craft-crw`](crates/crw/README.md): clean-room Canon CRW (CIFF) raw decoder, from LightCraft:
  the heap container, the entropy coding derived by black-box analysis (frozen code tables), sensor
  black and white levels and as-shot white balance. The complete derivation record (plan, sources
  log, scripts, verification) is in `crates/crw/clean-room/`. Colour stays in each app.
- [`craft-dialogs`](crates/dialogs/README.md): native file and message dialogs that never block
  the event loop: queued requests shown at the end of the frame and answered on a later frame,
  free-standing Open and Folder panels and Save sheets on macOS, case-insensitive filters. What
  to do with the answer stays in each app.
- [`craft-doc`](crates/doc/README.md): reads PhotoCraft `.pcraft`, VectorCraft `.vectorcraft`
  and EffectCraft `.ecproj` documents without the apps: the kind and version, the previews and
  PDF they carry, and their pages and layers. Rendering and editing stay in each app.
- [`craft-docpath`](crates/docpath/src/lib.rs): decides whether a path or `file:` URL stored in a
  document may be opened. Paths stored in documents are followed only when they are local or on the
  document's own network share; other network paths, device paths and non-file URLs are refused.
- [`craft-ecproj-format`](crates/ecproj-format/README.md): the envelope of EffectCraft `.ecproj`
  project files: `savedBy`, the schema version and its check, and the file object with the
  project and its saved view as type parameters. The project model stays in EffectCraft.
- [`craft-font-catalog`](crates/font-catalog/README.md): the installed fonts on every platform
  (font folders, fontconfig and Flatpak host folders, CoreText, DirectWrite, the font store),
  lookup by family, style, localized, legacy, full and PostScript names, family loads shared
  between threads, and rescans when fonts are installed or removed.
- [`craft-font-manager`](crates/font-manager/README.md): Craft Font Manager as a library: search
  and filters over the Google Fonts index, installed fonts by family, the font store interface,
  and (`egui` feature) `FontsPanel`, the whole interface as one panel for the launcher's Fonts tab
  and the standalone app.
- [`craft-font-store`](crates/font-store/README.md): the suite's font store: the pinned Google
  Fonts and Noto indexes plus indexes the user adds (never curated), exact lookup by family and
  style or PostScript name, and installs from each source's URLs (jsDelivr, then raw GitHub)
  checked by size, sha256 and parsing, with the licence, a manifest, and removal. Off until the
  user turns downloading on; agents can't.
- [`craft-fonts-build`](crates/fonts-build/README.md): embeds the optional craft-fonts faces
  from an app's `build.rs` (`CRAFT_FONTS_DIR`, `CRAFT_FONTS_REQUIRED`, script filters) as a table
  of `EmbeddedFace`s. No dependencies; the app picks the faces.
- [`craft-i18n`](crates/i18n/README.md): translations over the embedded craft-i18n catalogs:
  lookup with fallback (`pt-br` → `pt` → English), plurals, language names, the system locale and
  a pseudo locale. The catalogs stay a fully optional build input; apps keep `tl!` and their
  language setting.
- [`craft-jpeg`](crates/jpeg/README.md): clean-room decoder for 12-bit and arithmetic-coded
  JPEG (T.81 SOF0/SOF1 at 12 bits, SOF9, SOF10), from PhotoCraft, and a frame probe for routing.
  Metadata stays in each app.
- [`craft-keymap`](crates/keymap/README.md): keyboard bindings: chords and their text forms,
  scopes, conflicts, user overrides stored through craft-settings and import of other
  applications' shortcut sets. Command catalogs, file readers and the shortcut editor UI stay
  in each app.
- [`craft-audio`](crates/audio/README.md): audio file I/O and codecs (WAV, AIFF, FLAC, AAC),
  streaming and seeking, and packet decoding for any demuxer. See its README for formats and
  features.
- [`craft-media`](crates/media/README.md): bit-level I/O (`bitstream`), clean-room MP4/MOV
  (`isobmff`) and Matroska/WebM (`matroska`) demuxers and muxers, an Ogg demuxer (`ogg`), from
  FilmCraft, and an ISO BMFF box reader. Containers only; codecs are in `craft-audio`, and
  video codecs will go in a separate crate.
- [`craft-menu`](crates/menu/README.md): an app's menus as a macOS menu bar: the Mac layout
  (app menu, Window, Help, system-key clashes), a platform backend trait, and key equivalents
  fed back to egui as the input egui-winit would have made. Commands and their enabled state
  stay in each app.
- [`craft-pcraft-format`](crates/pcraft-format/README.md): the envelope of PhotoCraft's `.pcraft`
  bundle: `Manifest<D>` with the document as a type parameter, `TileRef`, the entry paths, the
  manifest decode limits and the migration runner. The document model stays in PhotoCraft.
- [`craft-print`](crates/print/README.md): PDF submission to CUPS and Windows, printer discovery,
  duplex/copies/colour and driver preferences. PDF generation stays in each app.
- [`craft-proof`](crates/proof/README.md): proofing per language (spelling, suggestions, grammar,
  hyphenation, a Hunspell reader, user words, Language packs). The word lists and patterns are
  craft-i18n's `language-data/`, embedded at build time from `CRAFT_I18N_DIR`: English and the
  carry-over locales by default, the set `CRAFT_PROOF_EMBED` names otherwise (`en` for English
  only); other languages come from installed Language packs.

- [`craft-quit`](crates/quit/src/lib.rs): native macOS quit, egui close handling and an optional
  app menu. Unsaved-document decisions and save prompts stay in each app.

- [`craft-settings`](crates/settings/README.md): `config.toml` settings storage for every app: platform folders, portable mode and overrides, atomic saves while running, backup, damaged-file recovery and schema migrations. Policy in [docs/settings-storage.md](docs/settings-storage.md).
- [`craft-text`](crates/text/README.md): text processing below the composer. Its `hyphen` module finds
  hyphenation points, with a bundled US English dictionary and patterns and optional hypher patterns
  for 47 more languages (one cargo feature each).
- [`craft-ui`](crates/ui/README.md): SVG catalogs, icon buttons, tabs, pane controls, transactional docking, print settings and theme helpers for egui frontends;
  assets, themes, localization and command handling stay in each app. Requires Rust 1.95 or later.
- [`craft-vectorcraft-format`](crates/vectorcraft-format/README.md): the envelope of VectorCraft's
  `.vectorcraft` file: its members, versions, embedded images and the data after the JSON, with the
  document as a type parameter. The document and its migrations stay in VectorCraft.
- [`craft-web-api`](crates/web-api/README.md): the page command API every Craft web build offers its page — one
  `window.<app>` shape (`request`, `execute`, `commands`, `docInspect`, `uiInspect`, `screenshot`) over the app's
  JSON control channel.
- [`craft-web-host`](crates/web-host/README.md): the host messages between a Craft app's web build and the page that hosts it (`?host=parent` or `data-host="page"`): one contract and one bridge for every app.
- [Shared UI component audit](docs/ui-components-audit.md): source inventory, extraction priorities
  and compatibility boundaries across the Craft suite.
- [Theme infrastructure](docs/theme-infrastructure.md): eight-app comparison and the small shared
  widget-state styling boundary.
- [Panel docking](docs/ui-docking.md): validated panel moves, floating groups, workspace adapters
  and an optional egui docking surface.

## References
- Desktop & plugin overview: [`apps/magic/README.md`](apps/magic/README.md) · Command & bridge contract: [`apps/magic/MAGIC.md`](apps/magic/MAGIC.md) · [`docs/control-protocol.md`](docs/control-protocol.md) · [`crates/engine/src/craft_hub/mod.rs`](crates/engine/src/craft_hub/mod.rs)
- Layer architecture & policy: [`docs/architecture.md`](docs/architecture.md) · [`xtask/src/policy.rs`](xtask/src/policy.rs)
