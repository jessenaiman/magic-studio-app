# Magic Studio

*Draft for content review. Screenshots are omitted from this text-only version.*

**Songs into animated films.**

A desktop workspace for the part between hearing a song and seeing its world.
Plan scenes, shape artwork, animate characters, and bring words and music into
the picture. The native app, CLI, and MCP share the same Rust engine.

[Platform: Linux source build](#install)
[ArtCraft suite: required](#made-with-artcraft)
[Mayhem: measured snapshot](#see-the-work-behind-the-work)

[Get started](#install) · [Production guide](MAGIC-STUDIO.md) ·
[Documentation](docs/README.md) · [Contribute](#contributing)

**Requires the [ArtCraft Crafting Apps suite](#made-with-artcraft).**

## From a song to a scene you can see

Start with a recording and lyrics. Bring scene timing, visual direction, artwork,
and continuity into one editable storyboard, with an underlying DeckCraft document
you can keep working on.

*Screenshot placeholder: All the Little Bunnies storyboard, Scene 01.*

*All the Little Bunnies, Scene 01. Song and supplied notes: Kathy Reid-Naiman.
The planned visual is a real project capture, cropped in PhotoCraft.*

> Jesse Naiman spent two days making his best storyboard by hand. In his own
> workflow, Magic Studio can bring that work down to a few minutes.

## Give the artwork a performance

Inspect a character on the stage, choose an authored action, and work with its
playback and motion curves. Keep the creative decisions close to the result.
The [animation workflow](MAGIC-STUDIO.md#2-animation-prepare-artwork-and-motion) covers
artwork preparation, motion authoring, and the remaining manual handoff.

*Screenshot placeholder: Babel character study in the Animation editor.*

*Babel character study, shown in the Animation editor. This is a separate project
from the Bunnies storyboard above.*

Continue through **Lyrics** to design the sung words, then **Studio** to work on
the linked HyperFrames composition. See the [production guide](MAGIC-STUDIO.md)
for the complete workflow and its prerequisites.

## See the work behind the work

Mayhem puts source measurements inside the app. Inspect what was scanned, how
large it is, and where complexity concentrates before deciding what to improve.

*Screenshot placeholder: Mayhem code measurements for the snapshot below.*

| Captured scan | Measured result |
| --- | ---: |
| Functions | 3,799 |
| Files with functions | 181 |
| Functions above CCN 15 | 0 / 3,799 |
| Maximum function CCN | 15 |

*Lizard 1.24.3 snapshot, 31 October 2026 UTC, Bunzen revision
[`57c08ab5`](https://github.com/jessenaiman/magic-studio-app/commit/57c08ab5f75b8e39d2a17f020892016354bae70e).
These figures describe that scan's measured scope, not the current main branch
or an overall quality grade.*

<details>
<summary>Screenshot provenance</summary>

The omitted screenshots are real desktop captures from 31 October 2026 UTC. The Bunnies storyboard
was captured from runtime `9291bf1-1793291146`; Animation and Mayhem were captured
from runtime `d730703-1793351806`. The Mayhem report measures the separate source
revision linked above. Crops preserve the original UI and have no added controls.

</details>

## Made with ArtCraft

**The [ArtCraft Crafting Apps suite](https://getartcraft.com/) is required.**
Magic Studio coordinates the production; ArtCraft supplies the creative tools
behind editable documents such as the DeckCraft storyboard. PhotoCraft was also
used to prepare the planned README screenshots.

Install and manage the suite with the official
[ArtCraft Launcher](https://github.com/storytold/craft-launcher).
Explore the [upstream open-source projects](https://github.com/storytold) and
support the artists and developers building them. The Crafting Apps suite is
separate from ArtCraft Studio, ArtCraft's generative image and video application.

## Install

The current source-installation path is for **Linux**; other platforms remain
unverified. Before building, install:

- The **[ArtCraft Crafting Apps suite](https://getartcraft.com/)**, installed through
  the [official Launcher](https://github.com/storytold/craft-launcher).
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

An installed app opens without a provider key. In addition to the required
ArtCraft suite, production workflows use HyperFrames; Blender rendering also
requires Blender, FFmpeg, and Linux user systemd. See the [production guide](MAGIC-STUDIO.md).
The [setup guide](agent-harness/cli_anything/magic/README.md) covers installer
options and development builds. Update native code or embedded UI by rebuilding
and reinstalling the reviewed revision.

## CLI and MCP

Use the same engine from the desktop, a terminal, or your MCP client.

<details>
<summary>CLI commands and MCP configuration</summary>

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

</details>

## Contributing

Focused fixes, documentation improvements, and reproducible bug reports are
welcome. Start with [CONTRIBUTING](CONTRIBUTING.md) and the
[architecture guide](docs/ARCHITECTURE.md). Keep changes scoped and report
the resulting behaviour, checks actually performed, and remaining limitations.
Use the source setup guide for development; rebuild after editing embedded UI.

## Credits

Magic Studio is a project by **Jesse Naiman**, incorporating and modifying
OpenMontage by calesthio and its contributors. See [source attribution](SOURCE-ATTRIBUTION.md),
[LICENSE](LICENSE), and component notices for upstream authorship and licensing.
