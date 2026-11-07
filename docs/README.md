# Magic Studio documentation

Magic Studio brings storyboard, animation, lyrics, and song-video composition using the artcraft design apps installed.

 Rule (TS-only, Rust server): Rust is the server side and only authority (engine, pipeline, documents, Craft integration, CLI/MCP/--control/API adapters). Frontend is TypeScript-React only — no handwritten JS. TS/TSX may present canvas, previews, and PhotoCraft editing, but only by emitting Rust registry commands and rendering engine state. Any logic that writes files, calls Craft/Codex/OpenAI, or owns document state must live in Rust. Migration path is JS → TS/TSX → Rust engine, never JS → Rust directly. Tauri v2 supports any frontend framework, so React-TS fits without changing stack.
- 1. Capabilities/ACL replaces allowlist (v2): No allowlist anymore. Permissions are granular capabilities/*.json + tauri.conf.json per window/webview, and build.rs must whitelist via AppManifest::commands(&["your_command"]) — by default all invoke_handler commands are callable from all windows. Why it matters: your TS React client must only be granted Magic CommandSpec IDs; otherwise a compromised frontend can call any Rust command.
- 2. Explicit Resource Management (2.12): await using file = await create(...) auto-disposes via Symbol.asyncDispose. Requires tsconfig.json: { target: "es2022", lib: ["es2022","esnext.disposable","dom"] } + Symbol.asyncDispose ??= Symbol(...) polyfill for older WebViews. Why it matters: TS file/asset handling in PhotoCraft previews must use this pattern or leak handles; JS sprawl hid this.
- 3. MSRV 1.90 + permission handler (2.12): Rust MSRV raised to 1.90 (stable-3 policy, Win7 dropped). New on_permission_request handler for webview camera/mic/geolocation/notifications. Why it matters: Rust server must build on 1.90+, and TS frontend cannot assume permissions — Rust must Allow/Deny/Default per PermissionKind, matching your --control token model.

## Choose a path

| I want to… | Start here |
|---|---|
| Understand the project | [Overview and native captures](../README.md) |
| Make a song video | [Production guide](../magic.md) |
| Run or build the app | [Authoritative setup guide](../apps/magic-gui/README.md) |
| Understand how the pieces fit | [Project context](../PROJECT_CONTEXT.md) |
| Browse Rust modules, types, and source | [Build the Rust API reference](RUSTDOC.md) |
| Use CLI or MCP commands | [Command architecture](../apps/magic-gui/MAGIC.md) and [Magic skill](../apps/magic-gui/SKILL.md) |
| Contribute safely | [Contribution and acceptance policy](../CONTRIBUTING.md) |


## Further reading

- [Project context](../PROJECT_CONTEXT.md): current layers and source contracts.
- [Agent guide](../AGENT_GUIDE.md): choose the right production layer.
- [Source attribution](../SOURCE-ATTRIBUTION.md) and [license](../LICENSE).
