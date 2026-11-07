# Magic Studio documentation

Magic Studio brings storyboard, animation, lyrics, and song-video composition using the artcraft design apps installed.

Rule (target state): Rust is the server side and only authority (engine, pipeline, documents, Craft integration, CLI/MCP/--control adapters). The frontend is TypeScript-React and presents canvas, previews and editing only by emitting Rust registry commands and rendering engine state. Any logic that writes files, calls Craft/Codex/OpenAI, or owns document state lives in Rust.

Current state: the Tauri shell still loads the preserved JavaScript workspaces from `apps/magic-cli/ui` (Project, Animation, Lyrics) inside same-origin frames beside the React views. The migration path is JS → TS/TSX → Rust engine, never JS → Rust directly.

Tauri v2 facts that shape this project (versions are in `apps/magic-tauri/Cargo.toml`; `Cargo.lock` pins them):

- **Capabilities and CSP.** Permissions are granular `capabilities/*.json`; the webview's Content-Security-Policy is `app.security.csp` in `tauri.conf.json` (`devCsp` for `tauri dev`). By default every `invoke_handler` command is callable from the window; `build.rs` does not yet restrict that with `AppManifest::commands`.
- **MSRV.** Tauri 2.12 requires Rust 1.90 or newer; this workspace requires 1.95.
- **Control.** Remote control is the token-authenticated `--control` channel documented in [control-protocol.md](control-protocol.md), not a Tauri webview permission.

## Choose a path

| I want to… | Start here |
|---|---|
| Understand the project | [Overview and native captures](../README.md) |
| Make a song video | [Production guide](../apps/magic/agent.md) |
| Run or build the app | [Tauri setup guide](../apps/magic-tauri/README.md) |
| Understand how the pieces fit | [Architecture](architecture.md) |
| Browse Rust modules, types, and source | [Build the Rust API reference](RUSTDOC.md) |
| Use CLI or MCP commands | [Control protocol](control-protocol.md), [command architecture](../apps/magic/MAGIC.md) and [Magic skill](../apps/magic/SKILL.md) |
| Contribute safely | [Architecture layers and rules](architecture.md) |


## Further reading

- [Architecture](architecture.md): current layers and source contracts.
- [Control protocol](control-protocol.md): `--control`, CLI bridge and MCP.
- [License](../LICENSE).
