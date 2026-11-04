# Backstage

Backstage is Magic Studio's Rust/Ratatui observer. Four views use the same
palette, tables and full-detail drawer: F1 Workshop, F3 MOPS, F4 Mayhem,
F5 Settings. It works at 80×24; below 40×10 it shows compact navigation help.

```sh
cargo run -p backstage
magic-cli audit --quick
backstage --audit-report target/analytics/analyzers.json
backstage --mayhem-report --audit-tool tsc
```

Arrows or j/k select; Tab changes focus; Enter/o opens the selected target;
p opens its folder; d opens complete details; PgUp/PgDn scroll details;
r rereads observations; q exits. Mouse tabs, row selection and explicit Open
are supported. `--color=always` overrides inherited NO_COLOR for this process.
Navigation never launches an app or runs an analyzer. Opener completion wakes
the existing UI event channel, so its status appears without another keypress.
Only the latest Open request can update that status. Opening requests the
system association; success does not prove that an application opened.

Desktop Mayhem's explicit Launch Backstage control uses the companion binary
beside the desktop executable. Build `cargo build -p magic-studio -p backstage`.
The standalone command remains available. Desktop Stop/exit and q end the session.

## Workshop and Settings

Workshop presents saved projects and their existing file references. Tab switches
project/file focus; Enter on the project pane opens its .magic file and p opens
its folder. 1–4 collapse creative-area groups; selection and Open follow visible
rows. Settings presents observed
configuration and role documents, with full paths and provider outcomes under d.
Saved references are not a complete filesystem census or an active agent cast.
Missing data is unavailable, never fabricated.

Official isolated `magic-cli --session-file /dev/null exec …` calls observe
`project.root`, `crew.list`, `project.catalog` and `backstage.observe`.
The obsolete `storyboard.template` query has been removed. The existing repository
template asset remains labeled as a source file, not a successful native query.
No project is attached or loaded by these observer calls. Each provider child
has a 10-second deadline, 1 MiB output bounds and its own process group; timeout
and exit reap only that group. One refresh runs at a time and input stays live.
No timer polling, generic agent/model execution or configuration mutation is added.

Mayhem inventory uses current-directory Git worktree/tracked-file observations,
not recursive home/temp scans. Samples retain existing bounds (16 worktrees,
3,000 tracked references / 200 ms per worktree). Missing/unsafe paths and sample
limits remain partial/error states; tracked sizes exclude untracked files and
are never summed across worktrees. Herdr activity uses the shared engine's
read-only snapshot adapter, with verified workspace/project-directory filtering.
Missing context, names or IDs remain unknown. Full scoped activity is under d.
See [the native reports and Herdr contract](../../docs/analyzers.md).

## Mayhem reports

F4 and the Mayhem mouse tab use the same saved-report entry action. Press a to switch reports/inventory; r rereads
without running checkers. The tool table displays actual exit status; the selected
pane shows original output first. [/] or PgUp/PgDn scroll it, d gives full details,
and Enter opens the retained raw stdout artifact. Failed tools remain visible.
No second engine, grades, guessed coverage, synthetic findings or count deltas.

`--audit-report PATH` (also `--audit-candidate PATH`) selects a manifest.
`--audit-tool NAME` selects a recorded tool. `--mayhem-report` prints native
reports and exits without a terminal. Existing legacy JSON remains readable as
raw data, without reinterpretation. Input must be regular JSON up to 32 MiB.
Original files are never rewritten. Optional `--audit-baseline PATH` remains a
raw artifact input; it does not create normalized comparisons. Expected revisions
via `--audit-baseline-sha`/`--audit-candidate-sha` require a matching recorded clean
revision. A dirty checkout is not claimed as evidence of its HEAD alone.

## MOPS handoffs

MOPS retains PR selection, task/agent/next action/blocker, review, provenance and
full original PR body. r explicitly reads configured inputs; selection does not
fetch. It never discovers, resumes or controls an agent session.

```sh
backstage --mops-repo OWNER/REPO --mops-pr NUMBER
backstage --mops-repo OWNER/REPO --mops-pr-file native-pr.json
```

Live input uses bounded read-only `gh api --method GET repos/OWNER/REPO/pulls/NUMBER`;
repeat --mops-pr up to 20 times. Import accepts native REST PR JSON object/array.
Optional `--mops-reviews-file NUMBER=PATH`, `--mops-checks-file NUMBER=PATH` and
`--mops-provenance PATH` attach only matching head/provenance. Inputs are regular
JSON up to 4 MiB. Imports are labeled snapshots; r rereads the file, not GitHub.

Enter opens the validated PR URL; l opens a verified role file; p opens the
recorded available worktree; c copies recorded resume text; f copies actual
feedback; m shows that PR's report scope; d shows full provenance and body.
OSC52 copy is a request, not confirmed host clipboard success. Commands are never
executed; missing/control-containing/over-4-KiB commands disable copying.
The canonical handoff fields live in `.github/PULL_REQUEST_TEMPLATE.md`.
Recorded session/worktree/Resume verified fields are metadata, not live validation.
`--mayhem-pr-report NUMBER` prints the same base/head evidence scope without
substituting unrelated reports or treating CI as reviewer approval.

## Observed verification

The CLI/Backstage build and scoped Clippy pass in this cloud workspace. Native
80×24 terminal interaction covered PR #76 snapshot import, selection, full-detail
scrolling and saved analyzer output; q exited cleanly. The connector-provided PR
fields were explicitly converted to native import shape. This is not proof of
live `gh` authentication (the installed token is invalid). External Open,
Settings project.root succeeded; crew.list reported a missing department role
file and activity receipt writing failed on the cloud read-only home filesystem.
Clipboard success, desktop embedding and successful live Herdr activity remain
unverified. Herdr is absent and its caller/workspace context is unavailable here.
No tests were added. The user's disconnected PC was not modified.
