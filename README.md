# Magic Studio

**Songs into animated films.**

Plan scenes, animate artwork, design lyrics, and compose a music video in one
desktop workspace. The native app and CLI share the same Rust engine.

[Install](#install) · [Production guide](apps/magic/agent.md) ·
[Documentation](docs/README.md) · [Contributing](#contributing)

## Document ownership and save/load contract

These are product decisions for the ongoing refactor, not a claim that every
existing screen already implements them.

| Data | Authoritative file | Save owner |
|---|---|---|
| Magic project details, document paths and content links | `.magic` (JSON) | Magic Studio |
| Application preferences and machine-specific settings | `config.toml` (craft-settings) | Magic Studio settings |
| Editable storyboard | `storyboard.designcraft` (DesignCraft) | DesignCraft |
| Editable animations | `.ecproj` | EffectCraft |
| The project's one linked film | `.fcproj` | FilmCraft |

Choose **File → New Project** (`Ctrl+N`), or open an existing `.magic` project.
In **Storyboard**, bring together the recording, timed lyrics, scene descriptions,
and artwork. Browse the saved scene pages, then choose **Edit in DesignCraft**
to edit and save the storyboard document. The intended Storyboard view brings
together the recording, timed lyrics, scene descriptions and artwork through
the linked source document; the approved creative layout and how verses map
to scene pages must be specified explicitly — choosing a file format does not
approve a template.

## Animation: bring artwork to life
<!-- EDITORIAL: Insert a real Animation capture with a prepared character,
a selected action, its preview, and the controls used to produce that result. -->

Open **Animation** and select an imported character or prop.
Prepare cutouts, rigs, and motion with the installed Craft applications
(PhotoCraft, VectorCraft, EffectCraft, FilmCraft) and place transparent clips
into the project. See the [production guide](apps/magic/agent.md) for artwork
preparation, motion authoring, and composition handoff.

## Lyrics: make the words part of the picture
<!-- EDITORIAL: Insert a real Lyrics capture showing a designed sung line,
its artwork, and the editing controls. Use only a supplied or cleared excerpt;
retain the original wording and give the song/recording artist accurate credit. -->

Open **Lyrics**, select a sung line, and shape its lettering, colour, placement,
and motion. Give individual words emphasis, preview the treatment on the song
clock, and save the changes. Visual design preserves the source words and timing.

## Studio: bring the film together

Open **Studio** and choose **Start the studio** when it is off.
Arrange and preview the linked HyperFrames composition, bringing the song,
artwork, animation, and lettering together. Composition files stay in the project.
Fidget's project conversation is available from the top bar when you need help.

## Watch the finished film

The finished-film example and its verified production credits are pending.

## Install

The current source-installation path is for **Linux**; other platforms remain
unverified. Before building, install:

- Git and Rust/Cargo **1.95 or newer**.
- A native compiler and the platform libraries required by the Rust workspace.

Cargo fetches the workspace's Rust dependencies during a fresh build.
The desktop package is `magic` and the engine package is `magic-engine`.

### Build and install the app and CLI

```sh
# Existing checkout: build and rerun checks without installing
cargo build -p magic -p magic-cli --locked
# Run the Tauri desktop: apps/magic/README.md#run-the-desktop
cargo xtask ci
cargo xtask layers
```
Asset rights metadata is pending; SonarCloud, WASM and translations are not configured.

```sh
git clone https://github.com/jessenaiman/magic-studio-app.git
cd magic-studio-app
cargo install --locked --path apps/magic-cli
(cd apps/magic && bun install && bun run tauri build)   # frontend + release binary: target/release/magic
```

`cargo install` places `magic-cli` in `~/.cargo/bin`. The desktop is built by `tauri build` rather than
`cargo install`, which would produce a binary that loads the dev server instead of the bundled frontend;
copy `target/release/magic` where you want it. (`bundle.active` is off in `tauri.conf.json`, so no
installer package is made.) There is no `cargo xtask install`. Run the desktop as described in
[apps/magic/README.md](apps/magic/README.md#run-the-desktop).

Ensure `~/.cargo/bin` is on your `PATH`, then launch `magic` (with `--control <PORT>` and `MAGIC_CONTROL_TOKEN`
for live CLI/MCP control; see [apps/magic/README.md](apps/magic/README.md#run-the-desktop)).

References for the desktop build and Tauri shell:
- [Desktop setup & plugin table](apps/magic/README.md) · [Control protocol & MCP](docs/control-protocol.md) · [Layer architecture](docs/architecture.md)
- Upstream references: [ArtCraft Tauri dev setup (`storytold/artcraft`)](https://github.com/storytold/artcraft/blob/main/_docs/dev_setup.md) · [Tauri v2 capabilities (`v2.tauri.app`)](https://v2.tauri.app/security/capabilities/)

Open **File → Application Settings** and choose an existing **Production project
root** outside the checkout. This selects the destination for new projects;
it does not move existing ones.

An installed app opens without a provider key. Shared foundation crates come from
[`storytold/craft-libs`](https://github.com/storytold/craft-libs), while sibling Craft applications
(DesignCraft, EffectCraft, FilmCraft, PhotoCraft, VectorCraft, DeckCraft, TextureCraft) are driven
through [`CraftHub`](crates/engine/src/craft_hub/mod.rs) over `--control` or their installed `<app>-cli`
binaries on `PATH`. Opening a separate native editor requires its installed
application. HyperFrames and FFmpeg remain external runtimes. See the
[production guide](apps/magic/agent.md).
The [setup guide](apps/magic/README.md) covers build
options and development builds. Update native code or embedded UI by rebuilding
and reinstalling the reviewed revision.

## CLI and Live `--control` / `--bridge`

`magic-cli` shares the Rust command registry with the desktop app. Omit `--bridge` to run headlessly in-process, or pass `--bridge 127.0.0.1:<PORT>` (with matching `MAGIC_CONTROL_TOKEN` in the environment) to drive a running `magic --control <PORT>` window with no silent headless fallback:

```sh
# Headless (in-process engine session)
magic-cli --help
magic-cli commands
magic-cli --project /absolute/path/Song.magic exec project.identity

# Live desktop control & MCP bridge (modeled on FilmCraft & EffectCraft)
export MAGIC_CONTROL_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
./target/release/magic --control 9876 --view project --project /absolute/path/Song.magic &
magic-cli --bridge 127.0.0.1:9876 ui inspect
magic-cli --bridge 127.0.0.1:9876 exec project.save
magic-cli --bridge 127.0.0.1:9876 mcp
```

References for what the CLI, `--control`, `--bridge`, and MCP are doing:
- Local contracts: [Control protocol & MCP](docs/control-protocol.md) · [Command & bridge contract](apps/magic/MAGIC.md) · [Magic command guide](apps/magic/SKILL.md) · [`CraftHub` bridge](crates/engine/src/craft_hub/mod.rs)
- Upstream Craft references: [FilmCraft control protocol (`storytold/filmcraft`)](https://github.com/storytold/filmcraft/blob/main/docs/control-protocol.md) · [FilmCraft CLI `--bridge` (`apps/filmcraft-cli/src/main.rs`)](https://github.com/storytold/filmcraft/blob/main/apps/filmcraft-cli/src/main.rs) · [EffectCraft MCP (`storytold/effectcraft`)](https://github.com/storytold/effectcraft/blob/main/docs/mcp.md) · [EffectCraft control protocol (`storytold/effectcraft`)](https://github.com/storytold/effectcraft/blob/main/docs/control-protocol.md) · [Shared foundation crates (`storytold/craft-libs`)](https://github.com/storytold/craft-libs)

## Contributing

Focused fixes, documentation improvements, and reproducible bug reports are
welcome. 
[command architecture](apps/magic/MAGIC.md). Keep changes scoped and report
the resulting behaviour, checks actually performed, and remaining limitations.
Use the source setup guide for development; rebuild after editing embedded UI.

## Credits

Magic Studio is a project by **Jesse Naiman**, incorporating and modifying
OpenMontage by calesthio and its contributors. See [LICENSE](LICENSE) and component notices for upstream authorship and licensing.
