# Magic Studio control inventory

Every element a client can drive on the live Tauri app, by view. Generated from the running app
with `ui.elements` and `ui.elements {unidentified: true}` (see [control-protocol.md](control-protocol.md)),
on a production-protocol build with a freshly created, **empty** project. Views that only show
controls when a song, assets or a storyboard exist (Lyrics designer, timeline lanes, storyboard
pages, asset tiles, Theatre curves) list fewer controls here than a populated project will;
re-run the two calls on a populated project to extend this list.

The Craft rule is that every interactive widget has a stable ID. Elements listed as
**Missing ID** are interactive but reachable only by coordinates (`ui.click {x,y}`).

## Shell (every view)

| ID | Element | Control |
|---|---|---|
| `brand` | button | Closes the project and returns Home |
| `tab-project` `tab-storyboard` `tab-design` `tab-animation` `tab-studio` | buttons | Switch view (`ui.set {view}` does the same) |
| `app-footer`, `footer-status` | footer, text | Status line |

`ui.set {view}` accepts `home`, `project`/`projects`, `storyboard`, `design`/`lyrics`, `animation`, `studio`.
Without a project the app stays on Home.


## Home (no project)

| ID | Element | Label | State |
|---|---|---|---|
| `home-new` | button | New Song Project |  |
| `home-open` | button | Open .magic… |  |
| `new-name` | input |  |  |
| `new-audio` | input |  |  |
| `new-root` | input |  | value `/home/jesse/.local/share/magic/projects` |

**Missing ID (14):**

- 1× button[button] “Browse…” inside `new-form`
- 1× button[button] “Folder…” inside `new-form`
- 1× button[submit] “Create Project” inside `new-form`
- 1× button[button] “Cancel” inside `new-form`
- 10× button[submit] “Open Project” inside `recent`

## Project

| ID | Element | Label | State |
|---|---|---|---|
| `block-song` | button | Song |  |
| `song-edit-details` | button | Song details… |  |
| `song-upload` | button | Upload audio… |  |
| `song-attach-original` | button | Attach lyric sheet… |  |
| `song-import-transcript` | button | Import timed lyrics… |  |
| `block-assets` | button | Assets |  |
| `asset-import` | button | Add asset |  |
| `block-animate` | button | Animate |  |
| `project-close-apps` | button | Close apps opened by Magic |  |
| `sb-add-asset` | button | Add asset |  |
| `footer-instance` | button | Connecting… |  |
| `footer-stage-song` | button | Song |  |
| `footer-stage-lyrics` | button | Lyrics |  |
| `footer-stage-transcript` | button | Transcript |  |
| `footer-stage-storyboard` | button | Storyboard |  |
| `footer-stage-animate` | button | Animate |  |
| `footer-stage-studio` | button | Studio |  |
| `footer-server` | button |  |  |

**Missing ID (2):**

- 1× button[submit] “Collapse blocks rail” inside `blocks`
- 1× summary “Details” inside `song-transcript`

## Storyboard

| ID | Element | Label | State |
|---|---|---|---|
| `storyboard-open-editor` | button | Open in DesignCraft | disabled in this state |
| `storyboard-refresh` | button | Refresh saved document |  |
| `storyboard-export` | button | Export PDF | disabled in this state |
| `sb-prev` | button | ← | disabled in this state |
| `sb-next` | button | → | disabled in this state |
| `storyboard-new` | button | Create from the DesignCraft template |  |

**Missing ID (1):**

- 1× summary “CLI and live control” inside `view-storyboard`

## Lyrics (`design`)

No identified interactive elements in this state.

## Animation

| ID | Element | Label | State |
|---|---|---|---|
| `motion-object` | select | Artwork |  |
| `motion-remove-asset` | button | Delete asset from project and try again | disabled in this state |
| `motion-rebuild-rig` | button | Prepare artwork… |  |
| `motion-assets-head` | summary | Assets |  |
| `motion-full-review` | button | Drawing / motion review |  |
| `motion-toggle-3d` | button | Toggle 3D viewport |  |
| `motion-pen` | button | Pen · draw a motion path |  |
| `motion-path-visible` | button | Show or hide the motion path |  |
| `motion-grid` | button | Show or hide grid |  |
| `motion-fit` | button | Fit artwork |  |
| `motion-actual` | button | Actual size |  |
| `motion-minus` | button | Zoom out |  |
| `motion-plus` | button | Zoom in |  |
| `btn-generate-all-sheets` | button | Generate missing character sheets |  |
| `motion-play` | button | Play motion |  |
| `motion-rewind` | button | Return to start |  |
| `motion-step-back` | button | Previous frame |  |
| `motion-step` | button | Next frame |  |
| `motion-scrub` | input | Scrub motion | value `0` |
| `theatre-view-keys` | button | Keys |  |
| `theatre-view-curves` | button | Curves |  |
| `theatre-save` | button | Save motion | disabled in this state |
| `footer-instance` | button | Connecting… |  |
| `footer-stage-song` | button | Song |  |
| `footer-stage-lyrics` | button | Lyrics |  |
| `footer-stage-transcript` | button | Transcript |  |
| `footer-stage-storyboard` | button | Storyboard |  |
| `footer-stage-animate` | button | Animate |  |
| `footer-stage-studio` | button | Studio |  |
| `footer-server` | button |  |  |

**Missing ID (1):**

- 1× button[submit] “Open Project” inside `motion-stage-status`

## Studio

| ID | Element | Label | State |
|---|---|---|---|
| `film-assemble` | button | Assemble |  |
| `film-preview` | button | Preview |  |
| `film-frame` | button | Grab Frame (0.0s) | disabled in this state |
| `film-open` | button | Open in FilmCraft | disabled in this state |
| `film-export-format` | select | H.264 (.mp4)HEVC / H.265 (.mp4)Apple Pro … | value `h264` |
| `film-export-range` | select | Entire SequenceIn / Out RangeWork Area | value `entire` |
| `film-burn-captions` | input |  | value `on` |
| `film-caption-sidecar` | select | None.srt.vtt |  |
| `film-export` | button | Export Film | disabled in this state |

**Missing ID (3):**

- 1× button[submit] “Export Final Cut Pro FCPXML via filmcraft_engine” inside `view-studio`
- 1× button[submit] “Export OpenTimelineIO via filmcraft_engine” inside `view-studio`
- 1× button[submit] “Export CMX3600 EDL via filmcraft_engine” inside `view-studio`

## Defects found by the audit

1. **Duplicate ID `app-footer`.** The shell footer and the footer inside the Project and Animation workspace
   frames both use it, so `ui.click` / `ui.focus` resolve to whichever comes first. Rename one.
2. **Per-project buttons without IDs.** Each recent-project card on Home has an unnamed *Open Project* button (and
   the New Project form's Browse…, Folder…, Create Project and Cancel). Give them IDs such as `recent-open-<n>`,
   `new-browse`, `new-folder`, `new-create`, `new-cancel`.
3. **Export buttons in Studio** (FCPXML, OpenTimelineIO, EDL) and the Project rail toggle have no IDs.
4. **Engine commands the live app cannot run.** `ui.feedback`, `ui.components` and `ui.attention` are registered
   in the engine, but the Tauri shell sends every `ui.*` id to the webview, which rejects them. Remove them
   from the registry or implement them in `apps/magic/ui/desktop.ts`.
5. **Timing.** `ui.click` returns when the event has been dispatched, not when React has re-rendered; read
   `ui.elements` again before the next step (the New Project form needs about two seconds to appear).
