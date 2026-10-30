# Backstage — observer views

Magic Studio's separate Rust/Ratatui observer has Workshop (F1), MOPS (F3),
Mayhem (F4), and Settings (F5). Booth remains disabled.

Actual native captures: [MOPS PR handoff](../../references/backstage/mops-pr22.png)
and [Mayhem audit evidence](../../references/backstage/mayhem-pr30.png).
These show imported GitHub snapshots and historical native audit artifacts,
not live session state or an audit of the current checkout.

Launch from a prepared source-development environment:

```sh
cargo run -p backstage
```

On Linux, desktop Mayhem also has an explicit **Launch Backstage** terminal control. Build
`cargo build -p magicstudio-shell -p backstage` and launch
`target/debug/magicstudio-shell` from the intended repository directory. The
companion must sit beside the shell executable; no PATH or shell command is used.
Mayhem navigation preserves one live session; q, Stop, or desktop exit ends it.
Launch failures remain inline. The standalone command above remains supported.

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

## MOPS — PR-owned handoff, never a session controller

The approved visual reference is versioned at
`references/backstage/mops-task-handoff-v2.png`. MOPS reads only explicitly selected
PR metadata. It never contacts a harness, discovers sessions, resumes an agent or
executes a command supplied by a PR. Navigation does not fetch GitHub data. Press R
explicitly to read the configured source; a single in-flight read is not duplicated.

Live source: `--mops-repo OWNER/REPO --mops-pr NUMBER` (repeat PR flags, maximum20).
The supported native `gh api --method GET repos/OWNER/REPO/pulls/NUMBER` route is
read-only and uses the existing bounded child lifecycle. This cloud's gh is not
authenticated; no login, token injection or credential changes were performed.

Acceptance/import source: `--mops-pr-file PATH` consumes an untouched native GitHub
REST PR JSON object (or bounded object array). Optional native auxiliary inputs are
`--mops-reviews-file NUMBER=PATH`, `--mops-checks-file NUMBER=PATH` and repeatable
`--mops-provenance PATH`. Inputs are regular JSON files bounded to4MiB, not an app
archive. Head identity/provenance gates review/check attachment. The screen labels
**imported GitHub snapshot**, retrieval time and the limitation that R rereads files,
not the network. This is not fully connected/live MOPS. Missing metadata is Unknown.

The single canonical PR rule is `.github/PULL_REQUEST_TEMPLATE.md`'s `## MOPS
handoff` section. The obsolete lowercase duplicate template was examined, found
unreferenced, and removed. Agents record their own harness, session ID, worktree,
recorded resume command and next action/blocker there. No session logging or
extra session script is needed. Existing role files are neither moved nor created;
role Open is enabled only for a discovered existing crew role target. A reported
worktree is a resolved directory, not proof of its branch or live session state.

Controls: arrows/j/k select PR; Enter/o opens its validated GitHub URL; l opens the
verified role; p opens the recorded available folder; c requests copy of a
recorded resume command; f requests copying actual review feedback;
m opens the same PR's Mayhem evidence scope; d exposes full source fields, warnings
and original PR body. Copy uses Crossterm OSC52 only and reports a request, not
confirmed host clipboard success. Missing commands, control characters, commands
over 4 KiB, and known unsupported terminals disable copy. Harness, session ID, worktree, and the
legacy Resume verified field are recorded metadata, never live-session
validation or prerequisites for copying the displayed command. Session ID and
Resume command are literal text after the field colon and one optional space;
do not wrap these values in Markdown backticks. Additional spaces, backticks,
and shell redirection remain unchanged. No copied command is ever executed by
Backstage.

`--mayhem-pr-report NUMBER` with the same explicit MOPS inputs prints native PR
base/head scope and missing matching audit-artifact evidence. It never substitutes
unrelated whole-project counts, never treats CI as reviewer approval/code-audit
coverage, and never starts a checker. A late audit result is checked against the
currently selected PR revisions before it can replace the visible evidence.

Native manual acceptance has used real imported PR22/PR30 snapshots. Those runs
are not production acceptance, live GitHub integration, clipboard confirmation,
GUI/full-video evidence or successful session resume. No tests were added or run.
