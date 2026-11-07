# Tauri desktop foundation

The desktop entry point is `src/main.rs`; the React frontend is in
`../magic/ui`. Plugin installation does not migrate the egui workspaces.

Registered official v2 plugins:

| Plugin | Integration |
| --- | --- |
| dialog | Existing native file picker; selected paths are added to the filesystem runtime scope. |
| fs | Binary/text reads and writes for app data and dialog-authorized paths. Project persistence remains owned by the Rust engine. |
| clipboard-manager | Text/image read and write APIs available for editor actions. |
| window-state | Automatically saves and restores window state. |
| single-instance | Subsequent launches activate the main window. Launch arguments are not yet forwarded to the frontend. |
| opener | Existing external file/URL opener with default scopes. |
| store | Persistent settings API available to the frontend. |
| shell | Existing shell integration; executable scopes must be configured for frontend process spawning. |
| log | Existing application logging. |
| websocket | Existing WebSocket integration. |
| http | Fetch API installed and registered; destination scopes must be added when connecting services. No origins are currently allowed. |
| notification | Native notification API available; request OS permission through a user-facing action when needed. |
| global-shortcut | Registration APIs available; no system-wide shortcuts are registered automatically. |
| process | Exit/relaunch APIs available. |
| os | Platform information APIs available. |

Capabilities live in `capabilities/default.json` and apply to the main window.
JavaScript bindings are dependencies of `../magic/package.json` (single-instance
is Rust-only). Generated permission schemas are committed alongside this setup.

Window customization is part of Tauri core, not an additional plugin. Keep native
decorations until the custom titlebar has working drag regions and window controls.
In v2, window configuration belongs under `app.windows` in `tauri.conf.json`.

## Run the Tauri desktop

This directory owns the only Tauri configuration. Clap parses startup arguments;
MCP and scripting belong to the separate `magic-cli` executable. The frontend
`bun run tauri` script delegates here.

From this directory, after `bun install` in `../magic`:

```sh
export MAGIC_CONTROL_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
../magic/node_modules/.bin/tauri dev -- -- --control 9876 --view projects
```

Use the same token environment for the CLI/MCP process. From the repository root:

```sh
cargo build -p magic-cli
./target/debug/magic-cli --bridge 127.0.0.1:9876 ui inspect
./target/debug/magic-cli --bridge 127.0.0.1:9876 mcp
```

See [the control protocol](../../docs/control-protocol.md) for the supported
operations, MCP tools, headless mode, and retired invocation forms. Installed
binaries must be rebuilt/reinstalled before using the new interface.

The frontend and control listener share one engine session. Project opening runs
in Tauri setup, after single-instance detection. A duplicate launch activates the
existing window; use the explicit bridge to open another project or change views.

Project, Animation, and Lyrics use the preserved sources in `apps/magic-cli/ui`,
packaged by `apps/magic/vite.restored-workspaces.ts`. Their adapter calls the parent
Tauri session directly; it does not start the retired HTTP UI server. Visited
editors stay mounted across view changes to preserve drafts and selection, while
hidden playback pauses. Studio's React component is unchanged. The new storyboard
view continues to use DesignCraft.

Deferred: updater/signing, distribution, deep links, advanced authoring operations
whose native replacements are not yet available, and UI operations beyond those
documented in the control protocol.

Build checks:

```sh
# Repository root
cargo check -p magic-tauri -p magic-cli --locked
# apps/magic
bun run build
```

Build checks do not verify interactive window behavior. Live verification must
use the running Tauri app and the same explicit bridge used by MCP.
