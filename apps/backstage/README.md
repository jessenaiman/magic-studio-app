# Backstage — observer views

A clean Rust/Ratatui observer draft with Workshop (F1), Mayhem (F4) and Settings
(F5). Booth and MOPS remain disabled. No old Backstage implementation was reused.
Workshop populated-runtime acceptance is in progress; Mayhem acceptance follows it.

```sh
cargo run -p backstage
```

Recommended: 166×54, truecolor and Unicode. Use `--color=always` to explicitly
override a process-inherited NO_COLOR setting without changing the desktop theme. Arrows/j/k select files; Enter/o or
mouse Open requests the desktop association; p opens the observed project folder; d shows wrapped observations and
provider outcomes (PgUp/PgDn scroll); r refreshes deliberately; q quits. Smaller
grids scroll; below 90×28 compact help preserves keyboard selection/open/details.
Selection/navigation never launches an app or contacts agents/models.

## Observation boundary

On Linux, startup and explicit refresh attempt these three Settings-specific official
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
not an exact visual-match/full-acceptance claim, and acceptance is not implied by navigation availability.

References: [Ratatui initialization](https://docs.rs/ratatui/latest/ratatui/init/index.html),
[Crossterm event handling](https://ratatui.rs/tutorials/counter-app/_multiple-files/event/),
[Rust process groups](https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#method.process_group),
[safe nix killpg](https://docs.rs/nix/latest/nix/sys/signal/fn.killpg.html).

## Project and worktree scope

Workshop also reads `project.catalog` (newest-ten history, not a filesystem census)
and `backstage.observe` for each listed project using the same isolated CLI and
bounded child lifecycle. Saved response identity/source must match the requested
canonical project. File rows are existing saved references and their local metadata,
not a guessed complete inventory. Four creative-area headers remain present;
unclassified saved asset references are explicitly separated. No filename or model
assignment is fabricated. Tab changes, selection and collapse never start queries.
Tab changes keyboard focus between project/sidebar and file/worktree list; 1–4
collapse Workshop groups. Enter/o opens the selected file/folder only explicitly;
p matches selected-file folder Open, or project folder with project focus.

Mayhem observes only the explicit current-directory Git repository using native
`git worktree list --porcelain -z` and `git ls-files -z --cached`. Commands disable
pager, fsmonitor and hooks, inherit no caller project binding, and use the same
bounded process-group lifecycle. Metadata is sampled only on startup/explicit R:
3000 tracked references / 200ms per worktree, maximum16 worktrees; missing/unsafe
paths and limits are partial/error states. No recursive home/temp scan is used.
Symlink escapes are excluded; hardlinked bytes are deduplicated within each path.
Counts/bytes exclude .git and untracked files and are never summed across worktrees.
Folder names come from observed tracked paths, not assumed source/build categories.
Token usage remains unavailable without a real saved usage provider; no inference,
provider calls, agent connections or subscriptions are used to populate it.

Git contracts: https://git-scm.com/docs/git-worktree and
https://git-scm.com/docs/git-ls-files .

## MAYHEM native audit artifacts

No checker runs on browse. Supply existing native Codacy JSON report files using
`--audit-baseline PATH --audit-candidate PATH`, optionally paired
`--audit-baseline-supplement PATH --audit-candidate-supplement PATH`. Source Git
provenance can be caller-supplied with `--audit-baseline-sha HEX` and
`--audit-candidate-sha HEX`; it is labeled external input, never invented from HEAD.
Reports are bounded regular JSON files (32MiB each), parsed outside the TUI loop.
Audit artifact sessions do not start native project queries; other views explicitly
show that project observation was not requested.

Add `--mayhem-report` to print the same read-only audit comparison as plain Markdown
and exit without initializing the terminal or launching any process. Tool names and
native issueCount/errorCount/filesAnalyzed/status fields are retained. Supplement
counts/errors remain separate; count deltas are not quality scores. Primary errors,
skipped files, missing tools and changed scope are not erased by retries.

In the artifact TUI: arrows/j/k select native toolId; [/] select that tool's candidate
finding; d opens native details, PgUp/PgDn scroll, Enter/o requests an observed source
file externally. Artifact and source links are bounded metadata observations, not a
report archive. Existing JSON files are never modified or rewritten.

For an unchanged-rule-inclusive readout, add `--audit-tool ESLint9` with
`--mayhem-report`. This prints the ten largest remaining native patternId groups by
candidate issue-record count, including unchanged groups—not only largest deltas.
The filter also selects that native toolId initially in the artifact TUI. Display
labels are a bounded static mapping: e.g. JavaScript quality / ESLint9 and Unknown
names / ESLint9_no-undef. Technical IDs stay indented/dim on the next terminal row
(same cell size), and are retained alongside labels in Markdown. No counts, fields,
rule identities or original JSON files are rewritten; unknown IDs use their raw
name. Official rule documentation links accompany known remaining-rule labels.
