# Magic Studio setup

This is the authoritative guide for running an installed Magic Studio app and
building/installing it from this repository.

## Use the installed app

```sh
magic
magic-cli --help
magic-cli commands
```

`magic` opens the native desktop application; `magic-cli` is its generated
CLI-Anything command adapter. `magic-cli mcp` serves the same engine operations
over MCP. Neither CLI-Anything nor an agent skill installs the native app.
See [the Magic skill](skills/SKILL.md) for project selection and MCP setup.

An existing installation does not need a source checkout, editable Python
package, provider-key setup, or model download just to open. Configure a provider
only when the chosen production task needs it. Keep project media in the
user-selected production folder rather than in this source tree.

## Build and install from source

Run these commands from the repository root, in a prepared development
environment with Rust/Cargo compatible with `Cargo.toml`, the platform's Tauri 2
build prerequisites, Python 3.10+, `uv`, and Git already available:

```sh
cargo build -p magicstudio-shell --locked
python3 scripts/install_magic.py
```

The installer requires the built `target/debug/magicstudio-shell`. It installs
`agent-harness` as a regular package into an application-owned Python environment
under `~/.local/share/magic-studio/.venv`, using the Python running the installer
without downloading another Python. It copies the native shell and tracked
runtime resources, and creates `magic`, `magic-cli`, the desktop entry, and icon
under `~/.local`. Ensure `~/.local/bin` is on your `PATH`.

For a release build, use matching profiles:

```sh
cargo build -p magicstudio-shell --release --locked
python3 scripts/install_magic.py --profile release
```

`python3 scripts/install_magic.py --help` lists `--prefix`, `--python`, and
`--migrate-legacy`. A custom `--python` must name an existing environment; the
installer installs the harness and its dependencies there. `--migrate-legacy`
changes only recognized old Magic entry points/configuration and is an explicit
migration choice, not a prerequisite for a fresh installation.

Rebuild and reinstall after embedded UI or native changes. If modifying the
bundled frontend integrations, follow their [build instructions](../../../apps/magicstudio-shell/frontend/README.md)
before rebuilding the shell. These commands install Magic Studio only; they do
not install other creative apps, production providers, or GPU models.

## Production tools and development

The app, CLI, and MCP share the Rust command registry; discover current operations
with `magic-cli commands` rather than copying historical command tables.
[Command architecture](../../MAGIC.md) explains dispatch and ownership.

The repository also retains Python tools, `pipeline_defs/`, stage-director
`skills/`, and dependency files for reusable production workflows. Their
capability-specific dependencies are separate from native app installation.
Use [the agent guide](../../../AGENT_GUIDE.md) and actual tool preflight when a
task requires those workflows; there is no root Python-package or Make setup
step. For code changes, follow [CONTRIBUTING](../../../CONTRIBUTING.md).

[Source attribution](../../../SOURCE-ATTRIBUTION.md) · [License](../../../LICENSE)
