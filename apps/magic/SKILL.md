---
name: magic
description: Operate Magic Studio live through magic-cli; inspect views, save projects, verify visible results, and measure code changes in Mayhem.
---

# Magic Studio

## Discover and work live

Use this checkout's installed `magic-cli`. Read `magic-cli --help` and
`magic-cli commands FILTER` for the operation needed. Commands and parameters
come from the installed registry; do not invent them or assume another
checkout's pipeline exists here.

Work in the visible desktop app so Jesse can watch the production develop.
Continue the authorized task through ordinary production decisions; discovery
is the start of the work. Respect an explicit request to pause or discuss.

```bash
magic-cli studio open /absolute/Project.magic
magic-cli studio list
magic-cli ui feedback
magic-cli ui state
magic-cli ui elements
magic-cli ui look output=/absolute/review.png
```

Check feedback for the intended project/window and actual readiness. Open the
returned screenshot and inspect it. No responsive window means live UI work
has not been verified; diagnose the failure before claiming a visible result.

## Views and controls

Discover controls with `ui elements`, then use their returned IDs. Current
source view buttons are:

| View | Button ID |
| --- | --- |
| Storyboard | `tab-project` |
| Animation | `tab-animation` |
| Lyrics | `tab-design` |
| Studio | `tab-studio` |
| Resources | `tab-skills` |
| Mayhem | `tab-analytics` |

```bash
magic-cli ui click id=tab-animation
magic-cli ui look output=/absolute/animation-review.png
```

Confirm IDs in the live controls; installed UI can differ from source.
Inspect Studio's actual state and linked project before describing its renderer.
Use `ui set` for controls and the registry command for project operations.

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

Render a page to look at it, and export the document when it is finished:

```bash
magic-cli storyboard render --path /absolute/storyboard.designcraft --page 1 --out /absolute/page.png
magic-cli storyboard export --path /absolute/storyboard.designcraft --pdf /absolute/storyboard.pdf
```

## Save and verify the result

```bash
magic-cli project save
magic-cli ui feedback
magic-cli ui look output=/absolute/saved-review.png
magic-cli app errors limit=50
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
verification authorized by Jesse, use Craft's actual Cargo commands directly:
`cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings`.
Capture each native command's output and exit status without custom scores or
relabeling failures. Do not run the withdrawn frontend or acceptance checks.
No Craft CI wrapper has been installed over Magic's existing `xtask`.

Mayhem's `mayhem.evidence` command may read retained historical reports. It does
not execute a scan. Preserve those reports and user project data.

## Tool boundaries

Use `magic-cli` commands that chain the installed Craft apps. A missing command
is an integration gap: fix a small authorized gap or report its exact failure.
Do not replace PhotoCraft, VectorCraft, TextureCraft, EffectCraft, FilmCraft,
or Blender with a duplicate implementation. Preserve creative writing and
assets. Resume existing projects; do not create a second studio setup.
