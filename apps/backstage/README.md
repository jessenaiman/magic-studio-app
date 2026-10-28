# Backstage

An observer-only production desk for Magic Studio. **Backstage** is the application; **MAYHEM** is its live system instrument room. Design and integration context: [issue #2](https://github.com/jessenaiman/magic-studio-app/issues/2).

```sh
cargo run -p backstage --release -- --theme daylight
```

Use `1`–`5` or Tab for Overview, Booth, MOPS, Code and MAYHEM. `/` filters the video list. Arrows select a video; Enter inspects it in Booth. `R` refreshes the current catalog/project observation, `A` requests `magic-cli audit`, `T` toggles the two bundled themes, and `Q` exits. Page keys scroll the audit. `--reduce-motion` disables brief character reactions.

## What is observed

- Overview reads `project.catalog` through `magic-cli`. The provider currently reports the ten most recently visited projects, not the whole production library; the view displays that scope.
- Booth uses a fixed `magic-cli run` batch: `project.attach`, then `backstage.status` with `trace=false`. Attach loads saved metadata without claiming project ownership, recording navigation history or switching Studio. No `--project`, `project.open`, save, render, approval or agent-send command is issued. A cached storyboard still is displayed as coloured terminal cells; no image generation or preview render occurs.
- Code requests the Magic-owned `audit` command and displays its JSON. At the initial implementation baseline this command is absent. Its actual error is shown; analytics and doctor are not substituted. Productivity counts remain unreported until an agreed audit schema supplies them.
- MOPS subscribes to the documented Herdr JSONL API's agent-status events. It never scrapes agent output or sends messages. This is a Herdr API connection, not a handwritten MCP client. Outside Herdr, explicitly pass `--herdr-socket PATH`; without an observation provider activity is unknown. If the connection closes, restart Backstage to reconnect. Names and titles are supplied by the host, not inferred project assignments.
- MAYHEM reads Linux `/proc` CPU, memory, network and top-RSS process measurements once per second only while visible. CPU/network need two samples. It keeps at most 120 CPU samples and 12 process rows. Returning to another tab stops sampling.

All Magic reads use a separate caller binding. The app has no arbitrary command runner, agent prompts or production controls. `--snapshot VIEW` prints a fixed-size offline layout with no service access; empty fields in that output are not live measurements.

## Themes

The colour source is Omarchy `colors.toml`, not colours sprinkled through rendering code:

```sh
backstage --theme daylight
backstage --theme midnight
backstage --theme omarchy
backstage --theme herdr
backstage --theme /path/to/colors.toml
backstage --theme daylight --print-herdr-theme
```

The last command prints compatible Herdr `[theme]` / `[theme.custom]` sections generated from the same palette. It does not replace Herdr configuration. The two theme directories under `themes/` are ordinary Omarchy themes; install them under the user's Omarchy themes directory if desired. Launching Backstage never changes the desktop theme. `--theme omarchy` reads the active palette under `XDG_STATE_HOME/omarchy/current/theme/`; `--theme herdr` reads the configured base's Omarchy palette plus Herdr custom tokens. Unsupported/missing theme data is an explicit error.

Status meaning uses words and symbols as well as colour. For example, a theme may map its `red` token to green. Light and dark views keep the same data and interaction.

## Notifications and remaining provider contracts

`--watch PATH` watches the supplied marker file's directory using OS notifications. It does not install a producer, invent a marker format or alter Magic's shared dispatch. No routine app polling runs. Magic's eventual agreed dirty-marker producer/consumer protocol, including observer-call exclusion and acknowledgment, is an integration dependency tracked in issue #2. Until agreed, use explicit refresh. Existing theme files are watched too.

The CLI process is bounded to a 20-second observation timeout and an 8 MiB report. Pending app refreshes coalesce while a read runs. Rendering waits for input or observations; a character reaction lasts less than a second, and MAYHEM alone owns the system-sampling timer. No background process scan or agent polling runs on other views.

## Verification

```sh
cargo check -p backstage
cargo clippy -p backstage -- -D warnings
cargo fmt -p backstage --check
```

The first real PTY measurement (debug build, Linux, short 3–4 second windows) observed about 5.5 MiB RSS, 0 measured CPU ticks on idle Overview, roughly 1% of one core in MAYHEM, and 0 measured idle ticks after leaving. These are preliminary observations, not a performance guarantee or release benchmark. Live Herdr subscription, keyboard navigation, catalog and selected-project inspection were exercised in the task-owned preview pane. Full production audit/marker acceptance remains dependent on Magic's contracts.

Behavioral suites are not added in this PR: the current repository policy requires Jesse to observe the real UI and explicitly approve each workflow first. Development-time parser/filter checks were exercised before that policy landed, then removed from the submitted source. Compiler, formatting, lint and live terminal checks remain.
