# Magic Studio command architecture

`magic` launches the native Tauri application. `magic-cli` is its CLI-Anything Click adapter; no arguments print help. `magic-cli mcp` runs the official Python MCP SDK stdio server. There is no REPL, install banner, `mc` compatibility executable or aggregate Craft MCP server.

The source of command IDs, labels, parameter documentation, journaling and long-running metadata is `crates/engine/src/commands.rs`. The CLI derives its natural `domain verb` hierarchy from `command.list`; it does not maintain a second dispatch table. MCP exposes `command_list` and `command_run` with engine-defined parameters, plus explicit window opening. The SDK owns the protocol and negotiation; there is no hand-written JSON-RPC loop or fabricated protocol version.

Both adapters use `core/session.py` for project/window binding. A selected live project uses its private Unix socket and expected project/instance guards. An unopened project uses one native `--headless serve` engine session for the lifetime of its CLI invocation or MCP connection. It claims the project writer lease, retains transient state, executes `Session::execute` and saves mutations. EOF releases the lease; explicit window opening closes the native session before transferring ownership to the GUI. The transport is automatic. A pinned window that closes or changes projects is an error, not a fallback to a different writer. Explicit project selection is the only rebind path. Session binding writes are locked; Python does not create or save project documents.

UI project actions enter `apps/magicstudio-shell/src/project_dispatch.rs` directly. It retains the correlation token, pending-operation guard, project-state checks, native ownership, draft flushing, revision acknowledgements and initiating frontend refresh. The former subprocess round trip through the CLI and a TCP port is removed. The socket adapter uses that same guarded application dispatch. Window-specific `ui.*` operations are declared in the application registry and implemented by the shell; they fail honestly without a window.

Other installed apps are independent. Use their documented CLIs directly when their work is needed. Existing Magic domain operations that coordinate project assets remain Magic operations; do not duplicate app capabilities or add generic Craft wrappers.

The former `motion.recipe` engine command only spawned two scripts and required users to pick a port. Run the existing compiler and recipe renderer directly instead:

```bash
python3 scripts/compile_motion_specs.py /absolute/project/animation
python3 scripts/render_motion_recipe.py /absolute/project/animation/recipe/04-render-clips.jsonl --project /absolute/project/Example.magic --place /absolute/project/animation/recipe/05-place-in-video.jsonl
```

The renderer uses installed `magic-cli` and project selection, preserving its receipts, input hashes and output checks. Core render and placement operations remain in the shared registry.

See [the packaged Magic skill](cli_anything/magic/skills/SKILL.md) for executable usage and MCP configuration. The matching repo skill is `.agents/skills/magic/SKILL.md`. Historical architecture/handoff documents describe retired implementations and must not be used to restore them.

References: [CLI-Anything HARNESS](https://github.com/HKUDS/CLI-Anything/blob/main/cli-anything-plugin/HARNESS.md), [MCP architecture](https://modelcontextprotocol.io/docs/learn/architecture), [official Python SDK](https://github.com/modelcontextprotocol/python-sdk/tree/v1.x), [PhotoCraft's discovery/execution adapter](https://github.com/storytold/photocraft/blob/main/crates/automation/src/server.rs). PhotoCraft is a reference only, not a dependency or integration target for this refactor.

MCP dispatch uses an AnyIO lock and non-abandoning workers. Cancellation of a client request does not release the lock before native work and binding updates finish. Live transport uses registry long-running metadata for reply deadlines; a lost reply after submission is explicitly an unknown outcome and is never retried automatically.

Native `--headless run` is a stop-on-error batch; the adapter uses internal `--headless serve` for persistent requests so a validation error does not discard session state. Native pipe reads and writes have the registry-based deadline too. A lost/timed-out native session is unavailable until explicit `project open` recovery; recovery never replays the failed request or replaces a still-running engine. Discovery remains available.

### Saved-project observation

`magic-cli backstage observe path=/absolute/Project.magic` reads the specified saved project
file without selecting a project or Studio window. Discover it with `magic-cli commands backstage.observe`;
MCP `command_run` uses ID `backstage.observe` and `{"path":"/absolute/Project.magic"}`. Call it
without global `--project`, which explicitly changes the caller's binding. The output includes the
canonical `project`, `name`, saved `assets` and `workshop`, and `observation` with
`source: saved_project_file`, `sha256` of the bytes read, `observed_unix_micros`,
`live_unsaved_state: unknown` and `external_files: not inspected`. It is a point-in-time disk read,
not a live-window snapshot or a transactional snapshot of external assets. It accepts an existing
absolute Magic Studio project file up to 32 MiB; invalid paths, other formats, `trace` and other
unsupported parameters are errors. It does not attach, load assets, claim a writer, write activity,
change a session binding, or switch a window. It remains usable while a Studio owns the project.
This narrow observation is not the Backstage audit and does not provide release or productivity totals.
