# Magic Studio command architecture

Magic follows the Craft app split: one Rust engine whose `Session::execute(id, params)` is the
only dispatch, a thin desktop shell, and a CLI that runs either headlessly or against the live
desktop. `apps/magic` (Tauri v2, React frontend in `apps/magic/ui`) is the desktop app. `magic-cli` is a separate
executable; no arguments print help.

The source of command IDs, labels, parameter documentation, journaling and long-running metadata
is `crates/engine/src/commands.rs`. The CLI resolves `magic-cli <domain> <verb>` (hyphens for
underscores) and `magic-cli <domain.verb>` against that registry; it keeps no second dispatch
table.

There are exactly two ways to run a command, and the choice is always explicit:

- **Headless.** `magic-cli [--project /absolute/Project.magic] <command>` runs in this process's engine
  for the lifetime of the invocation: it holds a short-lived project sidecar lock, executes
  `Session::execute` and saves mutations.
- **Live.** `magic-cli --bridge 127.0.0.1:PORT <command>` sends the command over the authenticated
  control channel to a running `magic --control PORT` and shares that window's session. Both
  processes need the same `MAGIC_CONTROL_TOKEN`. A bridge never falls back to a headless engine and
  there is no automatic window discovery; `--project` cannot be combined with `--bridge` (use
  `exec project.open path=/absolute/Project.magic`). See [the control protocol](../../docs/control-protocol.md).

The per-window Unix-socket transport is removed. The explicit Tauri control bridge is the live transport.

Agents use `magic-cli` only. DesignCraft, PhotoCraft, EffectCraft, FilmCraft, VectorCraft, DeckCraft, and TextureCraft operations go through `CraftHub` (`--control` loopback TCP when open live, or their installed `<app>-cli` executables on `PATH`), with shared foundation crates (`craft-settings`, `craft-atomic`, `craft-doc` gated on `info.supported`, `craft-docpath`, `craft-audio`) from `storytold/craft-libs`. Separate native editor windows still use their installed applications. `magic-cli studio open /absolute/Project.magic` runs `project.open` (in the live window with `--bridge`);
`magic-cli studio list` runs `project.catalog`. Verification uses the authorized native Cargo
commands directly; the retired Magic audit/review/formatting runners are removed.

See [the Magic skill](SKILL.md) for executable usage and project selection. The matching repo
skill is `.agents/skills/magic/SKILL.md`. Historical architecture/handoff documents describe
retired implementations and must not be used to restore them.

Live transport uses registry long-running metadata for reply deadlines; a lost
reply after submission is an unknown outcome and is never retried automatically.

### Runtime errors

`magic-cli --project /absolute/Project.magic app errors` reads retained failures
from the existing Rust activity journal. Mayhem reads the same report through `app.analytics.runtime_errors`.
Use `after=<next_cursor>` and `limit=50` to page through retained failures.
Rust command errors include their source chain and request ID; frontend failures
are recorded at the Rust UI-feedback boundary. Reading does not clear errors.
The bounded journal is observation history, not an assertion that a failure is
still active or that an empty list proves a working workflow.

### Saved-project observation

`magic-cli backstage observe path=/absolute/Project.magic` reads the specified saved project
file without selecting a project or Studio window. Discover it with `magic-cli commands backstage.observe`;
Call it
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

`magic-cli backstage changes`
returns the producer's resolved `path`, `available`, `token`, `reason`, `coverage` and
`excluded`. It is read-only and does not create or clear the marker or change caller binding;
omit global `--project`. The path is `$XDG_RUNTIME_DIR/magic/changes.json`, falling
back to `$XDG_STATE_HOME/magic/changes.json` or `~/.local/state/magic/changes.json`.
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
