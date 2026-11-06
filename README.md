# Magic Studio

**Songs into animated films.**

Plan scenes, animate artwork, design lyrics, and compose a music video in one
desktop workspace. The native app and CLI share the same Rust engine.

[Install](#install) · [Production guide](magic.md) ·
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

**The storyboard format is not yet approved.** Compare DeckCraft and DesignCraft
for layered content, production fields, scene insertion and stable references,
pacing, and image/audio/video use before locking one canonical format here.
Existing `.deckcraft` files remain unchanged during this investigation. Preserve editable
objects, grouping, order and visibility so production overlays can be hidden to
reveal the original creative. A `.potx` is an input template, not the storyboard
project. `.pptx` is optional interchange through DeckCraft, not a second source
of truth. Do not silently change formats, migrate existing projects, or replace
the source document with a flattened image or PDF.

The Storyboard view follows the linked source document. Complete editing
opens that same document in its owning app. Magic's production controls dispatch
designer actions through `magic-cli` to the owning app. Display rendering belongs
to the document app. DeckCraft's `file.render` has been verified to return a view
in memory, without a Magic-managed export-file pipeline; the equivalent must
be verified for any other candidate. DesignCraft documents an equivalent
`ui.render` method, but it has not yet been exercised in this comparison.
This is a rendered document view, not a
promise to embed a full external editor. Refresh after source edits; retain
the source artwork and editable overlays.

Saving a Magic project writes only Magic-owned metadata and links. It does not
serialize, rewrite, render, export or implicitly save another app's document.
Native-document saves and exports are explicit actions owned by that app.
Store project-relative links where possible so projects can be shared. Keep
machine-specific preferences outside shared project files.

Rust reads these records into typed data structures; JSON is a serialization
choice, not a reason to combine project documents and global settings. Keep
their schemas small and versioned. Workflow prerequisites and command results
must be enforced by the application; Markdown instructions describe that
workflow rather than acting as its execution engine.

The format comparison must cover a production storyboard, not only a layered
picture: editable art and overlays; image and motion prompts; locations,
characters and asset references; verse/lyric cues; audio/video references;
duration and timecode; and inserting or moving scenes without breaking links.
Scene identity must remain stable when its display number or position changes.
Confirm whether audio/video must play in the source document or may open in
Magic/FilmCraft before selecting the canonical format. Placed-artwork internal
layers and separate document objects/layers are different capabilities and
must be tested separately. No candidate is approved by its feature list alone.

## Storyboard: turn a song into a scene plan
<!-- EDITORIAL: Insert a real Storyboard capture with authored scenes and artwork.
Use the natural app-view proportions; keep navigation and useful controls visible.
Give the image descriptive alt text and retain relevant artwork/song credits. -->

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
into the project. See the [production guide](magic.md) for artwork
preparation, motion authoring, and composition handoff.

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
- A native compiler and the platform libraries required by the Rust workspace.

Cargo fetches the workspace's Rust dependencies during a fresh build.
The native desktop package is `magic` and the engine package is `magic-engine`.

### Build and install the app and CLI

```sh
# Existing checkout: build, run, and rerun checks without installing
cargo build -p magic -p magic-cli --locked
cargo run -p magic
cargo xtask ci
cargo xtask layers
cargo xtask assets
```
Asset rights metadata is pending; SonarCloud, WASM and translations are not configured.

```sh
git clone https://github.com/jessenaiman/magic-studio-app.git
cd magic-studio-app
cargo build -p magic -p magic-cli --release --locked
cargo xtask install --profile release
```

The installer places the `magic` desktop app and the `magic-cli` executable in:

- Commands under `~/.local/bin`.
- App and runtime resources: `~/.local/share/magic`.
- A desktop entry and icon under `~/.local/share`.

Ensure `~/.local/bin` is on your `PATH`, then launch:

```sh
magic
```

Open **File → Application Settings** and choose an existing **Production project
root** outside the checkout. This selects the destination for new projects;
it does not move existing ones.

An installed app opens without a provider key. Supported Craft document operations
use linked Rust libraries. Opening a separate native editor requires its installed
application. HyperFrames and FFmpeg remain external runtimes. See the
[production guide](magic.md).
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
welcome. Start with [CONTRIBUTING](CONTRIBUTING.md) and the
[command architecture](apps/magic/MAGIC.md). Keep changes scoped and report
the resulting behaviour, checks actually performed, and remaining limitations.
Use the source setup guide for development; rebuild after editing embedded UI.

## Credits

Magic Studio is a project by **Jesse Naiman**, incorporating and modifying
OpenMontage by calesthio and its contributors. See [source attribution](SOURCE-ATTRIBUTION.md),
[LICENSE](LICENSE), and component notices for upstream authorship and licensing.
