---
name: storyboard
description: Read, create, update, split, combine and restore Magic Studio storyboard pages through shared magic-cli commands. Lyrics and recorded timing remain project inputs.
---

# Storyboard

Read the project's `intent.md` first. The saved native document is
`storyboard/movie-board.json` plus its referenced editable PDF/DeckCraft version.
Use the existing `studio-template.deckcraft` pipeline; do not generate a second template,
Markdown storyboard, transcript or timing model.

Select the intended project once with `magic-cli project open path=/absolute/Project.magic` or `magic-cli studio open /absolute/Project.magic`. Routing to its window or native engine is automatic. The CLI, MCP and left Storyboard rail use the same commands:

```sh
magic-cli storyboard create
magic-cli storyboard read
magic-cli storyboard read scene=1
magic-cli storyboard read page=1
magic-cli storyboard update page=1 field=story value='The story'
magic-cli storyboard create scene=1 word=w17 expected_revision='<revision>'
magic-cli storyboard update scene=1 edge=end word=w20 expected_revision='<revision>'
magic-cli storyboard delete scene=2 into=previous expected_revision='<revision>'
magic-cli storyboard update restore='/project/storyboard/history/VERSION/manifest.json' expected_revision='<revision>'
magic-cli exec storyboard.backgrounds
magic-cli exec project.search query='captain'
```

`read` returns the current revision, stable scene IDs, pages and recorded words.
Use those word IDs to split or trim scene coverage. Never retime lyrics. Deleting a scene
combines its coverage with an adjacent scene; the removed page remains in the previous
version returned by the command. Stale revisions and pending design edits stop grouping changes.

`storyboard.update` edits existing native text fields. Inspect `storyboard.design`
for their actual names and page geometry. `save=false` batches field edits into one
`storyboard.save_version`. Picture placement/removal uses `storyboard.place_asset` and
`storyboard.remove_placement`, followed by `storyboard.save_version`.

`storyboard.backgrounds` returns only images assigned to the Backgrounds & locations
section as `{images:[{asset_id,name,path}]}`. An empty section returns an empty array;
never substitute the project asset library. Keep character, prop and background art separate.

Document existence is not completion: readiness lists unfilled template fields and unsaved work.
Only the user approves a scene. Legacy imports are source material for the same native document.
