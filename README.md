# Magic Studio

[![UI TypeScript & JS](https://img.shields.io/badge/UI_TypeScript_%26_JS-48%2F48_files_%C2%B7_0_warnings-brightgreen)](https://github.com/jessenaiman/magic-studio-app/issues/40)
[![Tauri Desktop Shell](https://img.shields.io/badge/Tauri_Shell-8%2F8_files_%C2%B7_0_warnings-brightgreen)](https://github.com/jessenaiman/magic-studio-app/issues/42)
[![Rust Engine Lizard](https://img.shields.io/badge/Rust_Engine_Lizard-57%2F57_files_%C2%B7_0_warnings-brightgreen)](https://github.com/jessenaiman/magic-studio-app/issues/41)
[![Cargo Clippy](https://img.shields.io/badge/cargo_clippy--D_warnings-passing-brightgreen)](https://github.com/jessenaiman/magic-studio-app/issues/40)
[![Mayhem Analytics](https://img.shields.io/badge/Mayhem_Scan-0_Rust_%26_TS%2FJS_hotspots-brightgreen)](crates/engine/src/complexity.rs)

**Songs into animated films.**

Plan scenes, animate artwork, design lyrics, and compose a music video in one
desktop workspace. The native app, CLI, and MCP share the same Rust engine.

[Install](#install) · [Production guide](MAGIC-STUDIO.md) ·
[Documentation](docs/README.md) · [Contributing](#contributing)

## Storyboard: turn a song into a scene plan
<!-- EDITORIAL: Insert a real Storyboard capture with authored scenes and artwork.
Use the natural app-view proportions; keep navigation and useful controls visible.
Give the image descriptive alt text and retain relevant artwork/song credits. -->

Choose **File → New Project** (`Ctrl+N`), or open an existing `.magic` project.
In **Storyboard**, bring together the recording, timed lyrics, scene descriptions,
and artwork. Browse the scene pages and use **Save new version** to keep the plan.
The storyboard remains an editable DeckCraft document.

## Animation: bring artwork to life
<!-- EDITORIAL: Insert a real Animation capture with a prepared character,
a selected action, its preview, and the controls used to produce that result. -->

Open **Animation** and select a rigged character and an authored action.
In Blender mode, choose **Render preview** and inspect the movement.
The render retains an editable Blender scene and a transparent clip in the project.
See the [animation workflow](docs/workflows/reference-to-animation-video.md)
for artwork preparation, motion authoring, and the remaining manual handoff.

## Lyrics: make the words part of the picture
<!-- EDITORIAL: Insert a real Lyrics capture showing a designed sung line,
its artwork, and the editing controls. Use only a supplied or cleared excerpt;
retain the original wording and give the song/recording artist accurate credit. -->

Open **Lyrics**, select a sung line, and shape its lettering, colour, placement,
and motion. Give individual words emphasis, preview the treatment on the song
clock, and save the changes. Visual design preserves the source words and timing.

## Studio: bring the film together
<!-- EDITORIAL: Insert a real Studio capture of the same project's HyperFrames
composition and preview. Crop only irrelevant space; keep navigation readable.
Use fluid-width images and real-text captions throughout this walkthrough. -->

Open **Studio** and choose **Start the studio** when it is off.
Arrange and preview the linked HyperFrames composition, bringing the song,
artwork, animation, and lettering together. Composition files stay in the project.
Fidget's project conversation is available from the top bar when you need help.

## Watch the finished film

The finished-film example and its verified production credits are pending.
<!-- EDITORIAL: Insert a real finished video verified as made with Magic Studio:
a clickable thumbnail, title, watch URL, and verified song/recording credits.
Credit Jesse Naiman's video and selected artwork created with AI assistance
where applicable. Verify Magic Studio production provenance before adding it. -->

## Install

The current source-installation path is for **Linux**; other platforms remain
unverified. Before building, install:

- Git and Rust/Cargo **1.95 or newer**.
- Python **3.10+** and [uv](https://docs.astral.sh/uv/getting-started/installation/).
- [Bun](https://bun.sh/docs/installation) for embedded frontend integrations.
- Your distribution's [Tauri 2 Linux build dependencies](https://v2.tauri.app/start/prerequisites/#linux),
  including the native compiler and GTK/WebKitGTK libraries.

A fresh build downloads missing Rust, frontend, and Python dependencies.

### Build and install the app and CLI

```sh
git clone https://github.com/jessenaiman/magic-studio-app.git
cd magic-studio-app
cargo build -p magicstudio-shell -p backstage --release --locked
python3 scripts/install_magic.py --profile release
```

This installs **Magic Studio**, **`magic-cli`**, and its MCP server dependencies.
The installer creates an application-owned Python environment and installs:

- Commands under `~/.local/bin`.
- App, runtime, and Python environment: `~/.local/share/magic-studio`.
- A desktop entry and icon under `~/.local/share`.

Ensure `~/.local/bin` is on your `PATH`, then launch:

```sh
magic
```

Open **File → Application Settings** and choose an existing **Production project
root** outside the checkout. This selects the destination for new projects;
it does not move existing ones.

An installed app opens without a provider key. Production workflows require
separate tools: HyperFrames, relevant Craft applications, and Blender, FFmpeg,
and Linux user systemd for Blender rendering. See the [production guide](MAGIC-STUDIO.md).
The [setup guide](agent-harness/cli_anything/magic/README.md) covers installer
options and development builds. Update native code or embedded UI by rebuilding
and reinstalling the reviewed revision.

## CLI and MCP

The CLI uses the application's command registry:

```sh
magic-cli --help
magic-cli commands
magic-cli commands project
magic-cli studio open /absolute/path/Song.magic
```

The MCP entrypoint is `magic-cli mcp`. Add the installed server to your MCP
client; for clients accepting `mcpServers` JSON:

```json
{
  "mcpServers": {
    "magicstudio": {
      "command": "magic-cli",
      "args": ["mcp"]
    }
  }
}
```

The client must have `magic-cli` on its `PATH`. Otherwise, use the absolute path
returned by `command -v magic-cli` as `command`. This registers the installed
server; it does not install the app or open a window.

Use **`command_list`** for discovery, **`command_run`** to execute an operation,
and **`studio_open`** to open/select a visible project. The
[Magic command guide](agent-harness/cli_anything/magic/skills/SKILL.md) explains
project targeting, MCP configuration, and independent session bindings.

## Contributing

Focused fixes, documentation improvements, and reproducible bug reports are
welcome. Start with [CONTRIBUTING](CONTRIBUTING.md) and the
[architecture guide](docs/architecture/README.md). Keep changes scoped and report
the resulting behaviour, checks actually performed, and remaining limitations.
Use the source setup guide for development; rebuild after editing embedded UI.

## Credits

Magic Studio is a project by **Jesse Naiman**, incorporating and modifying
OpenMontage by calesthio and its contributors. See [source attribution](SOURCE-ATTRIBUTION.md),
[LICENSE](LICENSE), and component notices for upstream authorship and licensing.
