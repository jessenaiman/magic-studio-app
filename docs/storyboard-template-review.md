# Storyboard template — critical review and rework plan

Reviewed 2026-11-05: `crates/engine/templates/storyboard.designcraft` (blank, 4 spreads), plus the
filled `media/merriweather/nelly/storyboard.designcraft` (8 spreads) to see how the fields are
really used. Opened in DesignCraft 0.17.2 (control port 7979).

Order of work: **1. rework the template in DesignCraft → 2. rename labels to the CLI convention →
3. only then change magic-cli.** No new workflow steps; this only covers fields the workflow
already needs and references an agent would otherwise have to ask for.

## Agreed direction

- Every page is a single landscape page, white and ink-light.
- Standard sections: `title-page`, `overview`, `locations`, `scenes`, `pages`.
  - `overview`: synopsis, overall concept, and a one-shot video prompt covering the whole storyboard.
  - `pages`: any other category (Cast & World, Animated Lyrics, Animated Drawings, Character …).
- Animated Lyrics is one overview page with 4–8 example cards. Animated Drawings is one overview
  page with the same cards.
- A card is a preview image, a scene link and an optional prompt. When a page is full, adding a
  card creates a continuation page of the same type; nothing overflows the margins.
- Slots are typed `text`, `image` or `read-only`. Read-only slots are filled from a source: lyrics
  from the transcript, and context copied from a referenced page (location prompt and thumbnail,
  character thumbnails, D01 style line).
- Scene pages are self-contained: they carry a copy of the location prompt, thumbnails and style.
- magic-cli shows structure, slots, requirements and rules only, and never anything about visual
  design. It keeps a small verb set driven by the template's labels and has no hard-coded field
  tables. designcraft-cli or the MCP does everything else.
- Lyrics are never edited. They are inserted locked, verse and chorus only.

## What the Nelly storyboard shows (real usage)

| Observation | Meaning |
|---|---|
| Cover, World and Character spreads were left **completely blank**; only scene pages were filled | The non-scene pages hold too many fields. Scenes pointed at "C01" and "Ocean Stage" with no page behind them. |
| `exact_prompt` left at its default on **every** scene | Nobody can regenerate or extend a scene. "More exaggerated" is impossible without the prompt that made the image. |
| The same image appears 3× per scene: hero, RESULT and strip #2. The source plate appears 2×. | Redundant slots inflate the page and the work. |
| `section` slot held the *action* ("CIRCLE LEFT"); `title` held the location name | Labels didn't match intent, so agents improvised. |
| `purpose` held "Artist's printed action: circle left." | The liner-note action (CD booklet / songs db `actions`) has no slot of its own. |
| `overall_design`, `character_individual`, `asset_geometry` left at their defaults on every scene | They should be read-only copies or removed, not typed per scene. |
| Action "circle left", but the hero image shows the rabbit walking right | No slot states direction or facing, so the page can't catch the error. |
| Page numbers print "00"; header reads "[PROJECT] / STORYBOARD" on every page | Placeholders are never resolved; there's no automatic numbering. |
| Pencil staging sketch + location sketch at the bottom of each scene | **Works well.** This is the motion-drawing idea already; keep it. |
| Lyric line + timing (`0:02.24 – 0:06.12`) | **Works well.** Keep it, read-only. |

## Page-by-page critique of the template

### Spread 1 — Cover + Production overview
- Keep: title, key art, D01 overall design box, scene map strip.
- Problems:
  - "[subtitle / format]" and "Short-video format" contradict the 16:9 1920×1080 scene images. The
    output format has to be stated once, explicitly.
  - The Master Reference Register (6 rows) duplicates what Cast & World should be. Its row borders
    are uneven (rows 1, 3, 5 boxed; 2, 4, 6 not).
  - REV 00 and "[footer note]" are never used.
  - "PRODUCTION BOOK" badge is noise.
  - The scene map is fixed at 5 slots. Nelly already needs more.
- Missing: song identity and sources (title, artist, album/track, audio file, transcript file,
  lyrics/actions source), and the **one-shot video prompt**.

### Spread 2 — The World / L01
- Busy, as you said: material & scale, reference register, revision/reason, source→result, crop
  note, fit note, match note, handwritten note.
- Keep only: location ID + name, **background image prompt**, background plate, **composite example**
  (plate + this page's assets), asset cards (ID + thumbnail) used here.
- The "Fit proportionally. Approve any crop. Never stretch." rule is a real rule. Move it to the
  template's rules, not the printed page.

### Spread 3 — Character / C01
- Keep: original → animation-ready pose, exact prompt, ID + name.
- The geometry diagram (1024×1536, body ~1300, baseline 1440) is a pipeline constant. Make it one
  read-only line, not a half page.
- The right half is ~60% empty; it fits on a single page.
- "Silhouette rules [rule 1–3]" and "provenance / review status" were never filled. Cut them, or
  make them optional.

### Spread 4 — Scene
- Keep: scene ID, lyric cue + timing (read-only), action, staging sketches, exact prompt, result.
- Cut: the duplicate hero/result pair (one image), "Overall design summary" typed per scene
  (becomes a read-only copy), "Character / individual", "Asset geometry", "match note", REV, and
  the grey contact-strip band (wastes ink). Repeated reference media become cards.
- Add (from gaps below): liner action (read-only), previous/next scene IDs, direction/facing, and
  which Animated Lyrics card and Animated Drawing card the scene uses.

### Document structure (affects the rename step)
- Each "spread" is **one 1600×1000 page with a drawn centre line**, not two pages. Going to single
  pages means rebuilding, not just resizing.
- Only `[Basic Paragraph]` exists, so all text is styled locally. Create about 6 styles (Heading,
  Label, Body, Prompt, Caption, Lyric) so the layout stays consistent as pages are added.
- The parent page `A-Parent` is empty. Running header, footer and page numbers are repeated as items
  on every page. Move them to parents, one per section, with automatic page-number markers.
- One layer. Split it into a locked **Design** layer (rules, labels, frames) and a **Content** layer
  (slots). magic-cli only touches Content. Original art kept under an overlay uses object `states`.
- Swatch "Cream" is actually pure white (rename it Paper). Empty image slots fill with 92% grey
  ("Slot"), so a blank storyboard prints 36 grey boxes. Use no fill and a hairline border.

## Gaps: where an agent would stop and ask, or fail

These are references the workflow already depends on but no page records:

1. **Song sources.** Audio file, transcript file, and the lyrics/actions source (omhas.db /
   561-song xlsx / CD booklet PDF). Without them `lyrics` can't insert locked lyrics. → `overview`
   read-only.
2. **Liner-note action.** The artist's printed action is the main input for most scenes. → scene
   read-only `liner_action`.
3. **Exact prompt of every generated image.** It's the only way to make "S3 but more exaggerated".
   → required whenever the image slot is filled.
4. **Asset file identity.** A printed page shows a picture but not which file it is. → every image
   slot gets a read-only caption with the asset ID and file name.
5. **Output format.** Aspect ratio, resolution and fps, stated once. Today it conflicts (short-video
   vs 16:9). → `overview`, copied read-only to scenes.
6. **Dangling references.** A scene names C01 or L02 but no page exists. → `check` must report it.
7. **Section.** Verse 1 / chorus, read-only from the transcript (the slot was misused for the action).
8. **Continuity for inserts.** Previous/next scene IDs and their thumbnails, plus the character's
   direction/facing. Needed for "new scene between S3–S4".
9. **Animated lyric / drawing assignment.** Which card on the overview pages a scene uses. Without
   it the overview pages are disconnected.

## Proposed label convention (for the rename step)

`<section>.<slot>` with type and requirement carried by the template, e.g.

```
scenes.lyrics          read-only  source=transcript
scenes.liner_action    read-only  source=songs.actions
scenes.action          text       required
scenes.location        read-only  ref=locations   (copies prompt + thumbnail)
scenes.result          image      optional        (exact_prompt required when set)
pages.lyrics.cards.N.image / .scene / .prompt
```

## Decisions (2026-11-05)

1. Page size: **US Letter landscape** (11×8.5″, 792×612 pt).
2. Slot rules live **inside the template**: the label carries name/type/requirement, a note on the
   frame carries the rule text. No separate rules file.
3. Approval is for the **whole storyboard** (no per-scene status slot).

## Mock run: "Beep Boop, Better Than You" (2026-11-05)

Project: `media/mock-beep-boop/`. It has invented lyrics and timings (`liner-notes/`,
`beep-boop.transcript.json`), 8 codex images with their exact prompts (`art/`), the storyboard
(`storyboard.designcraft`) and `exports/beep-boop-storyboard.pdf`. Built headless with the
installed magic-cli: `new` → `add` (2 locations, 2 characters, 3 scenes) → `lyrics` (locked,
verse/chorus only, spoken line skipped) → `set` per page → `export`.

What the run showed (feed into the template rework):

1. **Locked lyrics are silently cut off.** A 4-line cue doesn't fit the one-line cue box ("We never
   lose socks and…"), and the cue prints with doubled opening quotes (`""`). The lyric box must fit
   a full scene cue, and `check` must report overflow.
2. **Exact prompts overflow their boxes** on scene and location pages. Real prompts run 8–10 lines.
   The prompt box needs room, and overflow needs reporting.
3. **37 template placeholders still print** after a full fill ([handwritten note], [sketch],
   [staging note], [Individual character], [match note], spare register rows, S04/S05 map slots,
   [footer note]). Optional slots must print empty.
4. **Character page shows the same image twice.** This workflow has no separate "original drawing"
   for a generated character. The original card also overlaps the pose card.
5. **Character description drift.** The prompt said three legs and the image has two. Later scene
   prompts kept saying "three feet". The character page's *image*, not its prompt, has to be the
   reference, and the character page needs a "corrections" line that scene prompts copy.
6. **Dangling reference.** The child appears in S02/S03 with no character page. Nothing warned.
7. **Image size.** codex image_gen returns 1672×941 for a 1920×1080 request. Record the real size
   and treat 16:9 as the rule, not the pixel count.
8. **CLI inconsistencies:**
   - `slots` takes `--document` while `set`/`add` take `--path`.
   - `add --archetype world` appends after the scenes unless `--at` is given.
   - Setting text on a shape slot fails with `story.get: no story` instead of naming the slot.
   - The `cover_songmap_N` thumbnails are untyped shapes ("other").
   - `storyboard.lyrics` was in the source but not in the installed binary.

## Opinionated image requests (how to ask the image agent)

The request file `media/mock-beep-boop/art/REQUEST.md` worked on the first try (8/8, consistent
style). Its shape should be generated by magic-cli from the storyboard, not handwritten:

1. **Rules block.** One native image-tool call per image; save to an exact absolute path; write the
   verbatim prompt to `<name>.prompt.txt` beside it; on failure write `<name>.error.txt` and carry on;
   reply with path + pixel size. NEVER: text, letters, logos, borders; no other paths; no other files.
2. **STYLE = D01, verbatim**, identical for every image, and placed before the subject.
3. **Order: characters → locations → scenes → key art**, because later images list earlier ones as
   references.
4. **Characters:** 2:3, 1024×1536, full body, plain off-white, limbs separated, nothing touching the
   edge (the character-page geometry rule).
5. **Locations:** 16:9, **empty**, leave the middle third open for characters.
6. **Scenes:** "references: <location file>, <character files>". The subject is the scene's action
   and liner action, plus the shot size.
7. Each slot maps to one request field, so the storyboard alone produces the request. The saved
   `.prompt.txt` goes straight back into the page's exact-prompt slot.
