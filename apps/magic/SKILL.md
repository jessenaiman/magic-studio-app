---
name: magic
description: Operate Magic Studio live through magic-cli; inspect views, save projects, verify visible results, and measure code changes in Mayhem.
---

# Magic Studio

## Discover and work live

Use this checkout's `magic-cli`. Read `magic-cli --help` and `magic-cli commands FILTER`
for the operation needed. Commands and parameters come from the registry; do not invent them
or assume another checkout's pipeline exists here.

Work in the visible desktop app so Jesse can watch the production develop. Continue the
authorized task through ordinary production decisions; discovery is the start of the work.
Respect an explicit request to pause or discuss.

The live app is `magic-tauri`, started with an explicit control port and a per-session token
(see [the control protocol](../../docs/control-protocol.md)). The bridge is never discovered
automatically: ask for or read the port, and use the same `MAGIC_CONTROL_TOKEN` as the app.

```bash
magic-cli --bridge 127.0.0.1:9876 exec project.open path=/absolute/Project.magic
magic-cli --bridge 127.0.0.1:9876 ui inspect
magic-cli --bridge 127.0.0.1:9876 ui elements prefix=tab-
magic-cli --bridge 127.0.0.1:9876 ui screenshot /absolute/review.png
```

`ui inspect` reports the actual view, project, alerts, preserved-workspace feedback and any
Content-Security-Policy blocks. Open the returned screenshot and look at it. No responsive window
means live UI work has not been verified; diagnose the failure before claiming a visible result.

## Views and controls

Discover controls with `ui elements`, then use their returned IDs. The view tabs are:

| View | Button ID |
| --- | --- |
| Home | `tab-home` |
| Project | `tab-project` |
| Storyboard | `tab-storyboard` |
| Lyrics | `tab-design` |
| Animation | `tab-animation` |
| Studio | `tab-studio` |

```bash
magic-cli --bridge 127.0.0.1:9876 ui click tab-animation
magic-cli --bridge 127.0.0.1:9876 ui screenshot /absolute/animation-review.png
```

Controls: `ui click {id | x,y}`, `ui move`, `ui drag from= to=`, `ui scroll`, `ui key Ctrl+K`,
`ui type text=…`, `ui set id=… value=…` (or `checked=true`), `ui set view=animation`, `ui focus`,
`ui resize width= height=`, `ui screenshot`. Input is delivered as DOM events (no `:hover`, no
native drag-and-drop, no cross-origin frames). Confirm IDs in the live controls; the UI can differ
from source. Inspect Studio's actual state and linked project before describing its renderer.
Use the registry command for project operations; a delivered click is not proof, so read back state
and pixels.

## Storyboard documents

A storyboard is one `.designcraft` file made from the neutral template. Its
pages come in four kinds — cover, world (look development), character study and
scene — and every editable frame carries a script label, so the page layout
stays fixed while the text and images inside it change. Discover the pages and
their labels before editing:

```bash
magic-cli storyboard pages --path /absolute/storyboard.designcraft
magic-cli storyboard slots --path /absolute/storyboard.designcraft --page 1
```

Add a page copied from a template page, add a section (a parent page) and
apply it to pages, then fill the new page:

```bash
magic-cli storyboard add --path /absolute/storyboard.designcraft --archetype world --at 2 \
  --text world_heading="THE DESERT / L02"
magic-cli storyboard section --path /absolute/storyboard.designcraft --name B --based-on A-Parent --page 4
magic-cli storyboard set --path /absolute/storyboard.designcraft --label world_register \
  --text world_register="L01 Ocean Stage / stage-ocean-v1.png / 1672 x 941 px"
```

Set text and images with `--text SLOT=VALUE` and `--image SLOT=PATH`; target a
page with `--page`, `--label` or `--scene`.

Put new art over an existing slot without losing the original: the revision
becomes a new object on its own layer, and the frame it covers stays in the
document, hidden, so the earlier art can be shown again to compare.

```bash
magic-cli storyboard overlay --path /absolute/storyboard.designcraft --page 4 \
  --slot hero_image --image /absolute/revision.png --layer "Revision 01"
```

Pull the song's words from the canonical lyrics sheet — the artist's printed
lyrics — not from a recording. A transcript only supplies timing, and each
printed line takes its first occurrence, so a recording that sings the chorus
twice contributes one set of lines, not two. Only verse and chorus sections are
pulled:

```bash
magic-cli storyboard cues --lyrics /absolute/nelly.lyrics.json \
  --transcript /absolute/nelly.transcript.json --per 2
```

The output carries the sheet's printed structure, the timing of every line, and
the cues grouped the way the storyboard's scenes group them. The recording's
own repeated structure is reported separately as `as_sung`; it is never pulled
into the lines.

Find the content a page is still missing before editing it. A field that
still holds the template's default has never been filled; each one is reported
with the command that fills it, so a missing block traces straight to the CLI
call that changes it:

```bash
magic-cli storyboard check --path /absolute/storyboard.designcraft
```

Render a page to look at it, and export the document when it is finished:

```bash
magic-cli storyboard render --path /absolute/storyboard.designcraft --page 1 --out /absolute/page.png
magic-cli storyboard export --path /absolute/storyboard.designcraft --pdf /absolute/storyboard.pdf
```

## Save and verify the result

```bash
magic-cli --bridge 127.0.0.1:9876 exec project.save
magic-cli --bridge 127.0.0.1:9876 ui inspect
magic-cli --bridge 127.0.0.1:9876 ui screenshot /absolute/saved-review.png
magic-cli --bridge 127.0.0.1:9876 exec app.errors limit=50
magic-cli backstage observe path=/absolute/Project.magic
```

Inspect save errors, the screenshot, and the saved observation. The observation
reports disk state only; it does not verify unsaved UI or external assets.
For animation, play and inspect the output as well as checking its file exists.
A screenshot, successful command, or empty error list alone proves no movie.
After UI code edits, rebuild the shell before checking the installed app;
report if the visible app still runs a different build.

## Native Rust verification

The retired Magic audit, review and formatting wrappers are removed. For
verification authorized by Jesse, run `cargo xtask ci` (which executes
`cargo fmt --check`, `cargo xtask layers`, Clippy with `-D warnings`, tests across
the Tauri and egui package groups, and the `magic-tauri` control-protocol tests;
GitHub CI invokes the same gate through `storytold/craft-common-actions`).
Capture each native command's output and exit status without custom scores or
relabeling failures. Do not run the withdrawn frontend or acceptance checks.

Mayhem's `mayhem.evidence` command may read retained historical reports. It does
not execute a scan. Preserve those reports and user project data.

## Tool boundaries

Use `magic-cli` commands that chain the installed Craft apps. A missing command
is an integration gap: fix a small authorized gap or report its exact failure.
Do not replace PhotoCraft, VectorCraft, TextureCraft, EffectCraft, FilmCraft,
or Blender with a duplicate implementation. Preserve creative writing and
assets. Resume existing projects; do not create a second studio setup.
