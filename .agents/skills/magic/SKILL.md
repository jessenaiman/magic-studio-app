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

## Mayhem and every check-in

Before and after code changes, run the same relevant checks with matching
scope/configuration. The full audit command is:

```bash
magic-cli audit /absolute/checkout --base <same-comparison-commit>
magic-cli app analytics scan=false
magic-cli ui click id=tab-analytics
magic-cli ui look output=/absolute/mayhem-review.png
```

Audit saves measurements in `target/analytics/`, including `audit.json`.
Report check passes/failures, findings, functions over CCN 15, maximum CCN,
and long files where measured. Compare before/after and explain regressions;
Jesse expects measurable improvement, not merely another commit. Record
unavailable checks honestly. Never claim improvement from missing or
incomparable measurements. Preserve reports for Mayhem to display.

## Tool boundaries

Use `magic-cli` commands that chain the installed Craft apps. A missing command
is an integration gap: fix a small authorized gap or report its exact failure.
Do not replace PhotoCraft, VectorCraft, TextureCraft, EffectCraft, FilmCraft,
or Blender with a duplicate implementation. Preserve creative writing and
assets. Resume existing projects; do not create a second studio setup.
