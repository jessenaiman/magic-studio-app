# Magic Studio setup

This is the authoritative guide for running an installed Magic Studio app and
building/installing it from this repository.

## Use the installed app

```sh
magic
magic-cli --help
magic-cli commands
```

`magic` opens the native desktop application (`magic-studio`). `magic-cli` is
its command line, one Rust binary over the same engine registry; `magic-cli mcp`
serves the same operations over MCP. Neither installs the native app.
See [the Magic skill](SKILL.md) for project selection and MCP setup.

An existing installation does not need a source checkout, provider-key setup,
or model download just to open. Configure a provider only when the chosen
production task needs it. Keep project media in the user-selected production
folder rather than in this source tree.

## Build and install from source

Run these commands from the repository root, with Rust/Cargo compatible with
`Cargo.toml`, the platform's Tauri 2 build prerequisites and Git available:

```sh
cargo build -p magic-studio -p magic-cli -p backstage --locked
cargo xtask install
```

`cargo xtask install` (source in `xtask/`) requires those three built binaries.
It copies them and the tracked runtime resources under
`~/.local/share/magic-studio`, links `magic`, `magic-studio`, `magic-cli` and
`backstage` in `~/.local/bin`, and writes the desktop entry and icon. It
removes what earlier installs left behind: `magicstudio-shell`,
`python-magic-cli` and the retired Python CLI's environment. Ensure
`~/.local/bin` is on your `PATH`.

For a release build, use matching profiles:

```sh
cargo build -p magic-studio -p magic-cli -p backstage --release --locked
cargo xtask install --profile release
```

`cargo xtask install --help` lists `--prefix`. Rebuild and reinstall after
embedded UI or native changes. If modifying the bundled frontend integrations,
follow their [build instructions](../src-tauri/frontend/README.md) before
rebuilding the app. These commands install Magic Studio only; they do not
install other creative apps, production providers, or GPU models.

## Production tools and development

The app, CLI and MCP share the Rust command registry; discover current
operations with `magic-cli commands` rather than copying historical command
tables. [Command architecture](MAGIC.md) explains dispatch and ownership. For
code changes, follow [CONTRIBUTING](../../CONTRIBUTING.md).

[Source attribution](../../SOURCE-ATTRIBUTION.md) · [License](../../LICENSE)
