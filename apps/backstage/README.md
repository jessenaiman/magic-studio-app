# Backstage

A small TUI for **existing native analyzer reports**, plus explicitly requested
Herdr activity. The implementation is one file: `src/main.rs`. Actions, keyboard
and mouse mapping, and dispatch are together at its top. Drawing reads state and
returns fresh action targets; it never changes selection or runs a tool.

```sh
magic-cli audit --quick
backstage
backstage --audit-report /path/to/report.json
backstage --audit-report /path/to/report.json --audit-tool tsc --mayhem-report
```

The default is `target/analytics/analyzers.json` in the current repository.
`--audit-candidate` remains an alias for `--audit-report`. Input must be an existing
`magic.analyzers/1` JSON manifest, a regular file up to 32 MiB. Plain
`--mayhem-report` prints the original manifest, or the selected tool object with
`--audit-tool`. It never initializes a terminal or queries Activity.

The tool list displays actual exit statuses. Five views use the same text pane:

1. Output: original stdout, stderr and launch error.
2. Command: exact executable, arguments, scope, exit code and signal.
3. Run: recorded revision, dirty state, time, profile and source scope.
4. Files: artifact directory and complete additional native report content.
5. Activity: the existing native scoped Herdr adapter's response, including unknowns.

Arrows/j/k or a mouse row select a tool. 1–5, F1–F5, Tab or a mouse tab change
view. PgUp/PgDn scroll by the actual visible content height; [/] or the mouse wheel
scroll one line. Left/right scroll columns. d/Enter or the
Expand/close button expands/closes the text pane. Esc closes expansion, otherwise
quits. q or Quit exits. r/R or Reread refreshes the report, except in Activity,
where it explicitly queries `magic-cli exec mayhem.evidence` with
`activity_only:true`. No analyzer runs during browsing or refresh.

Activity retains the existing isolated Linux process-group lifecycle, 10-second
deadline, 1 MiB output limits, and cancellation on exit. It uses `/dev/null` for
session isolation and the existing Herdr schema/project/workspace filtering.
Other platforms can read reports; native Activity currently requires Linux.
No agent identities are inferred. No project, settings or report files are written.

Removed from this TUI: Workshop/project browsing, Settings/crew inspection,
MOPS PR imports/handoffs/resume/clipboard, Git inventory, report comparisons,
legacy audit JSON adapters, external file/browser launchers and embedded charts.
Existing project/report/history data is untouched. Their underlying application
commands remain available through magic-cli; Backstage no longer orchestrates them.
Desktop Mayhem and the shared analyzer runner remain separate, unchanged consumers.

Verified in real 45×12, 80×19 and 100×24 terminals: keyboard/mouse selection, five views, scrolling,
expand/close, report reread, Activity refresh and clean Quit. A real nine-tool
manifest kept its checksum; plain whole/selected reports matched native data.
Build and scoped Clippy passed. Herdr is absent here, so Activity honestly reports
unavailable caller context; successful live Herdr and desktop embedding remain
unverified. No new tests were added or run.
