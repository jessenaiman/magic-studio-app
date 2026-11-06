# Magic Studio setup

This is the authoritative guide for running an installed Magic Studio app and
building/installing it from this repository.

## Use the installed app

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

## Build and install from source

Follow the [Linux prerequisites](../../README.md#install) and run these commands
from the repository root:

```sh
npm ci --prefix apps/magic-cli
cargo build --locked -p magic-gui -p magic-cli
cargo xtask install
```

`cargo xtask install` (source in `xtask/`) requires both built executables.
It copies the executables and the tracked runtime resources under
`~/.local/share/magic`, links `magic` and `magic-cli` in `~/.local/bin`, and writes the desktop entry and icon. It
removes what earlier installs left behind: `magic-shell`,
`python-magic-cli` and the retired Python CLI's environment. Ensure
`~/.local/bin` is on your `PATH`.

For a release build, use matching profiles:

```sh
cargo build -p magic-gui -p magic-cli --release --locked
cargo xtask install --profile release
```

`cargo xtask install --help` lists `--prefix`. Rebuild and reinstall after native
changes when authorized. These commands install Magic Studio only; they do not
install other creative apps, production providers, or GPU models.

## Production tools and development

The app, Rust engine and CLI share the Rust command registry; discover current
operations with `magic-cli commands` rather than copying historical command
tables. [Command architecture](MAGIC.md) explains dispatch and ownership. For
code changes, follow [CONTRIBUTING](../../CONTRIBUTING.md).

[Source attribution](../../SOURCE-ATTRIBUTION.md) · [License](../../LICENSE)
