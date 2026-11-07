# Magic Studio frontend

This directory contains Magic's React frontend and the shared authoring UI.
The desktop entry point, Tauri configuration, and capabilities are in
[`../magic-tauri`](../magic-tauri/README.md).

## Develop and build

From this directory:

```sh
bun install
bun run tauri dev
bun run tauri build
```

The build produces `target/release/magic-tauri` at the repository root.
`bundle.active` is currently disabled, so it does not create an installer.
For authenticated CLI/MCP control and project startup arguments, follow the
[Tauri setup guide](../magic-tauri/README.md#run-the-tauri-desktop).

Install the separate CLI from the repository root:

```sh
cargo install --locked --path apps/magic-cli
```

There is no Rust desktop package in this frontend directory. Both Tauri IPC and
`magic-cli --bridge 127.0.0.1:PORT` use the desktop's engine session. Without a
bridge the CLI runs headlessly. Discover supported commands with
`magic-cli commands` and follow [the Magic skill](SKILL.md).

Keep artist media in the selected production project. Project persistence and
Craft integration belong to the Rust engine; the frontend renders state and
invokes registered commands.

[Command architecture](MAGIC.md) · [Contributing](../../CONTRIBUTING.md) ·
[Source attribution](../../SOURCE-ATTRIBUTION.md) · [License](../../LICENSE)
