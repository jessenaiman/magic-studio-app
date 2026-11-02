# Magic Studio command architecture

`magic` (the app under its short name) launches the native Tauri application, `magic-studio`.
`magic-cli` is one Rust binary (`apps/magic-cli`): no arguments print help, and `magic-cli mcp`
serves MCP over stdio with the official `rmcp` SDK. There is no REPL, install banner, `mc`
compatibility executable or aggregate Craft MCP server.

The source of command IDs, labels, parameter documentation, journaling and long-running metadata
is `crates/engine/src/commands.rs`. The CLI resolves `magic-cli <domain> <verb>` (hyphens for
underscores) and `magic-cli <domain.verb>` against that registry; it keeps no second dispatch
table. MCP exposes `command_list` and `command_run` with engine-defined parameters, plus
`studio_open`. `rmcp` owns the protocol and negotiation.

Both front ends use one caller session (`apps/magic-cli/src/session.rs`) for project/window
binding. A selected live project uses its private Unix socket with expected project/instance
guards (`crates/engine/src/studio_sessions.rs`). An unopened project runs in this process's
engine (`crates/engine/src/headless.rs`) for the lifetime of the CLI invocation or MCP
connection: it claims the project writer lease, retains transient state, executes
`Session::execute` and saves mutations. Opening a window drops that engine before the GUI takes
ownership. The transport is automatic. A pinned window that closes or changes projects is an
error, not a fallback to a different writer; when exactly one window has the pinned project
after a restart, the binding follows it. Explicit project selection is the only other rebind
path. The binding file is replaced atomically and never touches the `.magic` document.

UI project actions enter `apps/src-tauri/src/project_dispatch.rs` directly. It retains the
correlation token, pending-operation guard, project-state checks, native ownership, draft
flushing, revision acknowledgements and initiating frontend refresh. The socket adapter uses
that same guarded application dispatch. Window-specific `ui.*` operations are declared in the
application registry and implemented by the app; they fail honestly without a window.

Agents use `magic-cli` only; it calls the installed `*craft` CLIs internally. An agent's
`magic-cli studio open /absolute/Project.magic` opens the window on the emptiest of workspaces 1–4
without taking focus; `magic-cli studio list` finds open windows and `magic-cli studio previews`
lists HyperFrames preview servers. `magic-cli audit [PATH] [--tool NAME]` runs the repository's
own checks and prints their output unchanged.

See [the Magic skill](SKILL.md) for executable usage and MCP configuration. The matching repo
skill is `.agents/skills/magic/SKILL.md`. Historical architecture/handoff documents describe
retired implementations and must not be used to restore them.

References: [MCP architecture](https://modelcontextprotocol.io/docs/learn/architecture),
[rmcp](https://github.com/modelcontextprotocol/rust-sdk),
[PhotoCraft's discovery/execution adapter](https://github.com/storytold/photocraft/blob/main/crates/automation/src/server.rs).

MCP handles one command at a time: the session lock is held until native work and binding
updates finish. Live transport uses registry long-running metadata for reply deadlines; a lost
reply after submission is an unknown outcome and is never retried automatically.

### Runtime errors

`magic-cli --project /absolute/Project.magic app errors` reads retained failures
from the existing Rust activity journal. MCP uses `command_run` with ID
`app.errors`. Mayhem reads the same report through `app.analytics.runtime_errors`.
Use `after=<next_cursor>` and `limit=50` to page through retained failures.
Rust command errors include their source chain and request ID; frontend failures
are recorded at the Rust UI-feedback boundary. Reading does not clear errors.
The bounded journal is observation history, not an assertion that a failure is
still active or that an empty list proves a working workflow.

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

### Change notification token

`magic-cli backstage changes` (or MCP `command_run` with ID `backstage.changes` and `{}`)
returns the producer's resolved `path`, `available`, `token`, `reason`, `coverage` and
`excluded`. It is read-only and does not create or clear the marker or change caller binding;
omit global `--project`. The path is `$XDG_RUNTIME_DIR/magicstudio/changes.json`, falling
back to `$XDG_STATE_HOME/magicstudio/changes.json` or `~/.local/state/magicstudio/changes.json`.
The writer creates private files and atomically replaces the marker. An absent, invalid,
unreadable or nonprivate marker is unavailable, not a zero-change result.

Each observer watches the marker's parent, reads the token before and after refreshing, and
remembers its own last successfully observed token. Never delete or reset shared producer state.
A token is an opaque invalidation identity, not a revision counter; concurrent changes can
coalesce. Reconnect and rescan require a fresh observation. The first producer write creates the
marker; if its parent or watcher is unavailable, report that state and arrange a fresh watch
when available rather than asserting no changes.

Current producer coverage is limited to user-session attempts to write `.magic` project files through
`project::write_project_file` and updates to Magic's recent-project navigation history. Attempts
that fail after a partial write also invalidate; an invalidation does not prove bytes changed or
that an operation succeeded. External file edits, other project files, asynchronous work and
unsaved UI state and writes in non-user sessions are not covered. Publication is best effort: an error is written to the
producer's stderr without relabeling a completed save as failed. A process crash or publication
failure can miss an invalidation, and a previously readable token cannot prove the producer is
healthy. Consumers must report watcher/read failures and cannot infer full freshness from the
token alone.
