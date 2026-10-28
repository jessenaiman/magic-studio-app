# Save As response contract — awaiting Jesse's UI approval

This is a review specification and observation record, not an added test suite. Only after Jesse navigates/observes and approves the behavior should the test agent turn it into executable `magic-cli` command/expectation chains.

The shared command is `project.save` with an optional `path`. The desktop **File → Save Project As…** item (`CmdOrCtrl+Shift+S`) uses the native save dialog and the same command. Cancel submits no save; an intervening project switch refuses the request. The UI flushes pending storyboard text and refuses an unsaved timeline before changing identity. Destination writer protection remains in the engine.

Jesse must still approve the menu flow: cancel without changes, save to a new path, reject a destination held by another session, and ordinary Save after the transition. The older CLI observations below do not constitute approval of this new menu flow.

## Expected chain

| Action through installed `magic-cli` | Expected result | Expected visible/saved state |
| --- | --- | --- |
| `project open path=<Source.magic>` | Select the intended project/window | Header and `ui feedback` identify Source |
| While another session owns Destination, `project save path=<Destination.magic>` | Failure identifying the locked destination | Destination bytes unchanged; Source stays selected; no success UI acknowledgement |
| `project save path=<new-path.magic>` | Success with the canonical new path | Header, `ui state`, caller binding and saved file identify the new path |
| A second session attempts to claim the new path | Refused while the first session owns it | No competing write; saved bytes remain intact |
| A second session opens the old source | Allowed after successful Save As | Source ownership was released, and Source file was preserved |
| `project save path=<invalid destination>` | Failure | Current project and ownership remain unchanged |
| `project save` after that failure | Success | Current project saves normally |

Use disposable projects. Each step needs an expected command result plus visible state or saved-file evidence. Your user-driven UI approval is still pending; no TUI behavior has been asserted here.

## Implementation and observed checks

- `crates/engine/src/project.rs`: acquire a non-truncating destination writer lease before changing the selected project; preserve existing destination bytes on write failure; restore original path/owner on error.
- `crates/engine/src/lib.rs`: preserve the new lease when Save As changes project identity instead of restoring the old source lease over it.
- The prior installed build was observed overwriting a destination owned by a second `magic-cli` session, including changing its bytes.
- The fixed build refused that operation, preserved destination bytes and retained Source selection in a live MCP connection started by `magic-cli mcp`.
- Saving to a new nested destination succeeded. Another `magic-cli` caller was refused there; the old source could be opened by another caller.
- Saving to a directory failed without changing selection; a subsequent normal Save succeeded.
- A real desktop window followed a CLI Save As to `/tmp/magic-save-as-review/Save As Review.magic`; its reported project, header and caller binding matched. The disposable window is left open for Jesse's review.
- Native build and whitespace checks passed. No new tests or restored tests were added. Mid-write storage failure was not injected in this live check; the existing byte-restoration helper remains in use.

## Native menu observations (2026-10-28)

The Grober UI cleanup candidate was built and launched with `magic` using an explicit candidate
binary override and `/tmp/grober-ui-review/Source.magic`. AT-SPI activated the real native menu
and edited the real GTK file chooser; no dialog or engine mocks were used.

- Cancel preserved the source bytes and selected path.
- Save As to `Menu Copy.magic` created a readable project, changed the visible header, and reported
  the canonical destination with writer ownership and no engine/UI errors. Source bytes stayed unchanged.
- Reopening that saved destination succeeded.
- Save As to an existing file held by a separate OS writer lock was refused after the native overwrite
  confirmation; destination bytes and the current selection were preserved.
- Ordinary Save succeeded after that refusal. The candidate window was closed through guarded `app.quit`.
- Build, JavaScript syntax, source lint and whitespace checks passed. No new behavioral tests were added.

These engineering observations do not replace Jesse's pending UI approval. Concurrent project switching,
unsaved timeline behavior and storyboard draft flushing still need their own human review.
