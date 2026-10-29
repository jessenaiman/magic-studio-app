# Backstage — Settings first

A clean Rust/Ratatui Settings-only visual draft. F1–F4 remain disabled until their
individual design reviews. No old Backstage implementation was reused.

```sh
cargo run -p backstage
```

Recommended: 166×56, truecolor and Unicode. Arrows/j/k select files; Enter/o or
mouse Open requests the desktop association; p opens the observed project folder; d shows wrapped observations and
provider outcomes (PgUp/PgDn scroll); r refreshes deliberately; q quits. Smaller
grids scroll; below 90×28 compact help preserves keyboard selection/open/details.
Selection/navigation never launches an app or contacts agents/models.

## Observation boundary

On Linux, startup and explicit refresh each attempt exactly these three official
current-main CLI calls, in one background observation, without blocking UI input:

```
magic-cli --session-file /dev/null exec storyboard.template --params '{}'
magic-cli --session-file /dev/null exec project.root --params '{}'
magic-cli --session-file /dev/null exec crew.list --params '{}'
```

These are direct argument vectors, not shell strings. `/dev/null` is verified as a
character device: the CLI cannot inherit a saved project/window binding. No
`--project`, bridge, dry-run, project-open, discovery, agent/model call, or generic
command dispatcher is used. No repeat calls occur on tab/file selection. A refresh
in flight is not duplicated; the prior observation stays visible until completion.
Each call has a 10-second deadline and 1 MiB stdout/stderr limits. Only its own
isolated process group is killed/reaped on timeout or shutdown, including its
native backend. A blocking event channel wakes the UI; there is no polling timer.

The verified contracts are in `agent-harness/cli_anything/magic/magic_cli.py`,
`core/session.py`, `utils/magic_backend.py`, and the engine's `storyboard.rs`,
`project_settings.rs`, `crew.rs`, `commands.rs`. A blank native headless session
has no project to attach/load. The existing engine activity telemetry for journaled
read commands remains enabled under the user's logging authorization. Backstage
adds no logs, report archive, persisted context, session file, writer lease, or
configuration mutation; it does not link the engine or change desktop themes.

CLI/native-backend absence, timeout, malformed response and command errors remain
inspectable separately. Failed calls retain explicitly labeled repository fallback
for the existing native template and five actual department AGENTS.md files when
run inside this checkout. No embedded role fallback is passed off as an existing
file. Runtime replies provide template `path`, project `root`/`config`/`source`,
and crew `departments[].path`/`exists`/`document.department`; local metadata then
checks actual files. Missing root/config stays unavailable; no path is invented.
Role documents/candidates are never presented as active cast.

Total project count and unknown-file/folder classification remain unavailable:
there is no verified complete-library/classification contract. Desktop association
is unknown until explicitly requested. Open passes one canonical absolute target
argument and reports only launcher outcomes, never that an app actually opened.

## Acceptance status

The adapter is implemented from verified source contracts, but **real successful
CLI/native runtime verification remains blocked**: neither is installed in this
cloud workspace. The command-absent state can be exercised honestly; no fake
provider or records are used. External Open also remains unverified here. This is
not an exact visual-match/full-acceptance claim, and no later view is implemented.

References: [Ratatui initialization](https://docs.rs/ratatui/latest/ratatui/init/index.html),
[Crossterm event handling](https://ratatui.rs/tutorials/counter-app/_multiple-files/event/),
[Rust process groups](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#method.process_group),
[safe nix killpg](https://docs.rs/nix/latest/nix/sys/signal/fn.killpg.html).
