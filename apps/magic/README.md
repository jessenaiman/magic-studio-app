# Magic Studio desktop

This directory is the `magic` desktop app: the Tauri entry point (`src/main.rs`),
Tauri configuration, capabilities, and the React frontend with the shared authoring UI.

## Develop and build

From this directory:

```sh
bun install
bun run tauri dev
bun run tauri build
```

The build produces `target/release/magic` at the repository root.
`bundle.active` is currently disabled, so it does not create an installer.
For authenticated CLI/MCP control and project startup arguments, follow the
[desktop setup](#run-the-desktop).

Install the separate CLI from the repository root:

```sh
cargo install --locked --path apps/magic-cli
```

Both Tauri IPC and
`magic-cli --bridge 127.0.0.1:PORT` use the desktop's engine session. Without a
bridge the CLI runs headlessly. Discover supported commands with
`magic-cli commands` and follow [the Magic skill](SKILL.md).

Keep artist media in the selected production project. Project persistence and
Craft integration belong to the Rust engine; the frontend renders state and
invokes registered commands.

The React frontend is in `ui`. The engine owns documents and command dispatch; the frontend owns presentation.

Registered official v2 plugins:

| Plugin | Integration |
| --- | --- |
| dialog | Existing native file picker; selected paths are added to the filesystem runtime scope. |
| fs | Binary/text reads and writes for app data and dialog-authorized paths. Project persistence remains owned by the Rust engine. |
| clipboard-manager | Text/image read and write APIs available for editor actions. |
| window-state | Automatically saves and restores window state. |
| single-instance | Subsequent launches activate the main window. Launch arguments are not yet forwarded to the frontend. |
| opener | Existing external file/URL opener with default scopes. |
| store | Registered but **not granted**: user prefs/layout persist only in `config.toml` (`craft-settings`, one authority). Window position is `window-state`. No `LazyStore` JSON — add a scoped file only for ephemeral frontend-only state (never prefs/duplicates `config.toml`). |
| shell | Registered but **not granted**: `shell:default` was removed from the capability and the frontend does not use it, so the webview cannot spawn or open anything through it. Add a scoped permission when a feature needs one. |
| log | Existing application logging. |
| websocket | Existing WebSocket integration. |
| http | Fetch API installed and registered; destination scopes must be added when connecting services. No origins are currently allowed. |
| notification | Native notification API available; request OS permission through a user-facing action when needed. |
| global-shortcut | Registration APIs available; no system-wide shortcuts are registered automatically. |
| process | Exit/relaunch APIs available. |
| os | Platform information APIs available. |

Capabilities live in `capabilities/default.json` and apply to the main window.
JavaScript bindings are dependencies of `package.json` (single-instance
is Rust-only). Generated permission schemas are committed alongside this setup.

Window customization is part of Tauri core, not an additional plugin. Keep native
decorations until the custom titlebar has working drag regions and window controls.
In v2, window configuration belongs under `app.windows` in `tauri.conf.json`.

## Webview security

Set in `tauri.conf.json` and `capabilities/default.json`; verified against a production-protocol build
(`cargo build -p magic --features tauri/custom-protocol`) with every view open and no violations:

- **Content-Security-Policy** is enforced (`csp`; `devCsp` relaxes script and websocket sources for
  `tauri dev` only). Scripts are `'self'` plus WebAssembly; no `eval`, no remote scripts, no remote
  images or media. Embedded frames and fetches may reach loopback only (the HyperFrames preview
  server). Blocks are recorded in `ui.inspect` → `cspViolations`, and workspace frames report them as alerts.
- **`withGlobalTauri` is off.** The React app imports `@tauri-apps/api`; the preserved JavaScript
  workspaces get their `window.__TAURI__` from `ui/workspaces/tauri-adapter.js`, which forwards
  to the parent's session.
- **Asset protocol scope** starts with application data/cache directories only. Opening a project
  grants its directory through `allow_directory`; referenced external media and returned previews
  receive file grants. GLTF models also grant their directory for relative textures and buffers.
  Startup, frontend IPC, and the control bridge share this behavior. Grants accumulate until the
  app exits; switching projects does not revoke earlier grants. Hidden paths and credential-looking
  names remain subject to the configured deny patterns.
- **devtools** is a cargo feature (`--features devtools`), off for release builds; debug builds always include the inspector.
- **shell** is registered but not granted (see the plugin table).

## Run the desktop

This directory owns the only Tauri configuration. Clap parses startup arguments;
MCP and scripting belong to the separate `magic-cli` executable.

From this directory, after `bun install`:

```sh
export MAGIC_CONTROL_TOKEN="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
bun run tauri dev -- -- --control 9876 --view projects
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
packaged by `vite.restored-workspaces.ts`. Their adapter calls the parent
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
cargo check -p magic -p magic-cli --locked
# apps/magic
bun run build
```

Build checks do not verify interactive window behavior. Live verification must
use the running Tauri app and the same explicit bridge used by MCP.

[Command architecture](MAGIC.md) · [Contributing](../../CONTRIBUTING.md) ·
[Source attribution](../../SOURCE-ATTRIBUTION.md) · [License](../../LICENSE)
