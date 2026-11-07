# Magic Studio setup

For the current Tauri application, see the [Tauri setup guide](../magic-tauri/README.md).
This page describes the legacy egui application and CLI installation.

## Use the legacy egui app

```sh
magic
magic-cli --help
magic-cli commands
```

`magic` opens the native egui desktop application. `magic-cli` is
a separate headless executable backed by the same engine.
See [the Magic skill](SKILL.md) for project selection and CLI usage.

An existing installation does not need a source checkout, provider-key setup,
or model download just to open. Configure a provider only when the chosen
production task needs it. Keep project media in the user-selected production
folder rather than in this source tree.

## Build and install the legacy egui app from source

Run these commands from the repository root, with Rust/Cargo compatible with
`Cargo.toml`, the native platform libraries required by the Rust workspace and Git available:

```sh
cargo install --locked --path apps/magic
cargo install --locked --path apps/magic-cli
```

Cargo builds in release mode and places `magic` and `magic-cli` in `~/.cargo/bin`; put that folder on your
`PATH`. The earlier `cargo xtask install` (resource copy, `~/.local/bin` links, desktop entry) was removed: Magic
installs like any Cargo app. An installation made by the old installer under `~/.local/share/magic` is not updated
by these commands, and `~/.local/bin/magic-cli` may still point at it; remove those links or put `~/.cargo/bin`
first on `PATH`.

`cargo add -p magic <crate>` declares a desktop dependency. Cargo fetches and
compiles project dependencies during the build. Use `cargo install` for Magic's executables.

To connect the CLI or MCP server to a running Tauri window, start it with
`--control PORT` and use `magic-cli --bridge 127.0.0.1:PORT ui state` or
`magic-cli --bridge 127.0.0.1:PORT mcp`. Without `--bridge`, `magic-cli mcp`
starts a headless session.

`magic --lyrics` opens the Lyrics editor and offers a folder picker. Pass the
song folder directly as `magic --lyrics /path/to/song`. The folder must contain
its canonical lyric sheet and timed transcript. A `.magic` file is optional.

Rebuild and reinstall after native
changes when authorized. These commands install the legacy egui app and CLI, not the Tauri app; they do not
install other creative apps, production providers, or GPU models.

## Production tools and development

The app, Rust engine and CLI share the Rust command registry; discover current
operations with `magic-cli commands` rather than copying historical command
tables. [Command architecture](MAGIC.md) explains dispatch and ownership. For
code changes, follow [CONTRIBUTING](../../CONTRIBUTING.md).

[Source attribution](../../SOURCE-ATTRIBUTION.md) · [License](../../LICENSE)
