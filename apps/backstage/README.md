# Backstage

An observer-only production desk for Magic Studio. **Backstage** is the application; **MAYHEM** is its live system instrument room. Design and integration context: [issue #2](https://github.com/jessenaiman/magic-studio-app/issues/2).

```sh
cargo run -p backstage --release -- --theme daylight
```

Click the tabs or use `1`–`5`, F1–F5, Left/Right or Tab for Overview, Booth, MOPS, Code and MAYHEM. Click inside the Backstage pane first to give it keyboard focus. `/` filters the video list. Up/Down, mouse-wheel or row clicks select a video; Enter inspects it in Booth. `R` refreshes the current catalog/project observation, `A` requests `magic-cli audit`, `T` toggles the two bundled themes, and `Q` exits. Page keys scroll the audit. `D` opens the selected source record and `?` opens help and provider coverage. `--reduce-motion` disables character animation.

## What is observed

- Overview reads `project.catalog` through `magic-cli`. The provider currently reports the ten most recently visited projects, not the whole production library; the view displays that scope.
- Booth calls only `magic-cli backstage observe path=/absolute/Project.magic`, the narrow saved-file query supplied by Grober PR #12 (provider commit `1448d90`, based on PR #11, not yet public main at integration). No global `--project`, attach/status fallback or rich status inference is used. The consumer checks the canonical returned identity and saved-file observation source. It displays saved name/assets/workshop, reported byte hash and timestamp; live unsaved state and external files remain explicitly unknown/uninspected. If the provider is absent, the real error stays visible. Cached storyboard stills come separately from the catalog and are not covered by the observation hash.
- Code requests the Magic-owned `audit` command and displays its JSON. At the initial implementation baseline this command is absent. Its actual error is shown; analytics and doctor are not substituted. Release counts and history remain unknown until Jesse defines an authoritative release source; neither navigation history nor audit is assumed to be a release ledger.
- MOPS subscribes to the documented Herdr JSONL API's agent-status events. It never scrapes agent output or sends messages. This is a Herdr API connection, not a handwritten MCP client. Outside Herdr, explicitly pass `--herdr-socket PATH`; without an observation provider activity is unknown. If the connection closes, restart Backstage to reconnect. Names and titles are supplied by the host, not inferred project assignments.
- MAYHEM reads Linux `/proc` CPU, memory, network and top-RSS process measurements once per second only while visible. CPU/network need two samples. It keeps at most 120 CPU, memory and network samples and 12 process rows. Returning to another tab stops sampling.

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

Backstage discovers `magic-cli backstage changes` and uses the provider's resolved path; it does not guess a global marker location or clear the shared file. Before/after token reads bracket each app observation. A changed token or notification retains a separate refresh demand behind explicit reads. Each instance remembers its own last successful token. A newly installed/rearmed watch requires a fresh bracketed read, closing the startup gap. Atomic replacements are observed by watching the parent directory. If the parent does not exist yet, the nearest existing ancestor is watched for its creation; the UI explicitly reports unavailable notification state until the provider and watch are usable. Normal operation has no app polling.

PR #18 (`926620f`, stacked on #12 and #11) supplies the candidate marker provider. These are unmerged dependencies, not public-main functionality. The UI displays the provider's coverage and exclusions. Coverage is best effort, limited to the listed user-session project/history write attempts. A readable old token does not prove producer health or comprehensive freshness. Missing audit remains independent; a successful audit is not treated as acknowledgment of a project snapshot. `--watch PATH` remains an optional extra notification source and never creates or acknowledges producer state.

The CLI process is bounded to a 20-second observation timeout and an 8 MiB report. Pending app refreshes coalesce in a separate refresh-needed flag while a read runs. Queued explicit audit/project reads cannot erase it; it is consumed only when a catalog read is scheduled. A later change requires another refresh, and a failed read remains visible without an automatic retry loop. Rendering waits for input or observations; brief character reactions stop when idle; working performers animate only on visible MOPS (unless reduced motion is enabled), and MAYHEM alone owns the system-sampling timer. No background process scan or agent polling runs on other views.

## Verification

```sh
cargo check -p backstage
cargo clippy -p backstage -- -D warnings
cargo fmt -p backstage --check
```

The first real PTY measurement (debug build, Linux, short 3–4 second windows) observed about 5.5 MiB RSS, 0 measured CPU ticks on idle Overview, roughly 1% of one core in MAYHEM, and 0 measured idle ticks after leaving. These are preliminary observations, not a performance guarantee or release benchmark. Live Herdr subscription, keyboard navigation and catalog were exercised in the task-owned preview pane. The earlier selected-project inspection exercise did not establish absence of implicit writer attempts; that path was removed after review and is never used as a fallback. PR #12 is a required provider dependency for the new narrow query. Full production audit/marker acceptance remains dependent on Magic's contracts.

Behavioral suites are not added in this PR: the current repository policy requires Jesse to observe the real UI and explicitly approve each workflow first. Development-time parser/filter checks were exercised before that policy landed, then removed from the submitted source. Compiler, formatting, lint and live terminal checks remain.
