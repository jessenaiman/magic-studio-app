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
| Application preferences and machine-specific settings | Separate user `settings.json` | Magic Studio settings |
| Editable storyboard | Format decision pending DeckCraft / DesignCraft feature comparison | Selected document app |
| Editable animations | `.ecproj` | EffectCraft |
| The project's one linked film | `.fcproj` | FilmCraft |

Choose **File → New Project** (`Ctrl+N`), or open an existing `.magic` project.
In **Storyboard**, bring together the recording, timed lyrics, scene descriptions,
and artwork. Browse the saved scene pages, then choose **Edit in DeckCraft**
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
The native desktop package is `magic` and the engine package is `magic-engine`.

### Build and install the app and CLI

```sh
# Existing checkout: build and rerun checks without installing
cargo build -p magic-tauri -p magic-cli --locked
# Run the Tauri desktop: apps/magic-tauri/README.md#run-the-tauri-desktop
# (`cargo run -p magic` starts the legacy egui app)
cargo xtask ci
cargo xtask layers
cargo xtask assets
```
Asset rights metadata is pending; SonarCloud, WASM and translations are not configured.

```sh
git clone https://github.com/jessenaiman/magic-studio-app.git
cd magic-studio-app
cargo install --locked --path apps/magic-cli
(cd apps/magic && bun install && bun run tauri build)   # frontend + release binary: target/release/magic-tauri
```

`cargo install` places `magic-cli` in `~/.cargo/bin`. The desktop is built by `tauri build` rather than
`cargo install`, which would produce a binary that loads the dev server instead of the bundled frontend;
copy `target/release/magic-tauri` where you want it. (`bundle.active` is off in `tauri.conf.json`, so no
installer package is made.) There is no `cargo xtask install`. Run the desktop as described in
[apps/magic-tauri/README.md](apps/magic-tauri/README.md#run-the-tauri-desktop). The legacy egui `magic` app
installs with `cargo install --locked --path apps/magic`.

Ensure `~/.cargo/bin` is on your `PATH`, then launch `magic-tauri` (with a control port and token for
agent use, see the Tauri README).

Open **File → Application Settings** and choose an existing **Production project
root** outside the checkout. This selects the destination for new projects;
it does not move existing ones.

An installed app opens without a provider key. Supported Craft document operations
use linked Rust libraries. Opening a separate native editor requires its installed
application. HyperFrames and FFmpeg remain external runtimes. See the
[production guide](apps/magic/agent.md).
The [setup guide](apps/magic/README.md) covers installer
options and development builds. Update native code or embedded UI by rebuilding
and reinstalling the reviewed revision.

## CLI

The CLI uses the application's command registry:

```sh
magic-cli --help
magic-cli commands
magic-cli commands project
magic-cli studio open /absolute/path/Song.magic
```

The [Magic command guide](apps/magic/SKILL.md) explains project targeting
and independent session bindings. Craft document operations use their linked
Rust libraries where integrated. Magic retains its genuine command-line interface.

## Contributing

Focused fixes, documentation improvements, and reproducible bug reports are
welcome. 
[command architecture](apps/magic/MAGIC.md). Keep changes scoped and report
the resulting behaviour, checks actually performed, and remaining limitations.
Use the source setup guide for development; rebuild after editing embedded UI.

## Credits

Magic Studio is a project by **Jesse Naiman**, incorporating and modifying
OpenMontage by calesthio and its contributors. See [LICENSE](LICENSE) and component notices for upstream authorship and licensing.
