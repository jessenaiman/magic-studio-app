---
name: artcraft-suite
description: Use when a task involves any ArtCraft app (Magic Studio, PhotoCraft, EffectCraft, FilmCraft, VectorCraft, TextureCraft, FigCraft, DesignCraft, CADCraft, DeckCraft, GridCraft, LightCraft, PdfCraft, SoundCraft, WordCraft, Craft Anything) — picking the app, driving it, or checking what it can do. Learn from the apps, not the text.
---

# ArtCraft suite

Learn from the apps, not the text. Each app ships a live `--help` and a
command catalogue; both always match the installed binary, and both correct
you when you guess. Run them before driving an app; never trust remembered or
written tool lists — they go stale the moment an app updates.

One line per app — read the app itself:

- magicstudio — 2D/3D character animation, rigs, Blender export — `magic-cli --help` + `magic-cli commands`
- photocraft — raster image editor — `photocraft-cli --help` + `photocraft-cli commands`
- effectcraft — motion-graphics compositor — `effectcraft-cli --help` + `effectcraft-cli commands`
- filmcraft — NLE video editor — `filmcraft-cli --help` + `filmcraft-cli commands`
- vectorcraft — vector illustration, incl. imageTrace raster→vector — `vectorcraft-cli --help` + `vectorcraft-cli commands`
- texturecraft — 3D texture painting & mesh-map baking — `texturecraft-cli --help` + `texturecraft-cli commands`
- figcraft — Figma-class UI design — `figcraft-cli --help` + `figcraft-cli commands`
- designcraft — page layout & graphic documents — `designcraft-cli --help` + `designcraft-cli commands`
- cadcraft — 2D/3D CAD drafting — `cadcraft-cli --help` + `cadcraft-cli commands`
- deckcraft — presentations (.deckcraft/.pptx) — `deckcraft-cli --help` + `deckcraft-cli commands`
- gridcraft — spreadsheets, formulas, charts — `gridcraft-cli --help` + `gridcraft-cli commands`
- lightcraft — photo library & RAW developer — `lightcraft-cli --help` + `lightcraft-cli commands`
- pdfcraft — PDF edit/OCR/render — `pdfcraft-cli --help` + `pdfcraft-cli tools`
- soundcraft — multitrack audio editor — `soundcraft-cli --help` + `soundcraft-cli commands`
- wordcraft — word processor & document conversion — `wordcraft-cli --help` + `wordcraft-cli commands`
- craft-anything — agentic triage, investigation & rebuild — `craft-anything-cli --help` + `craft-anything-cli commands`

Solidcraft is third-party (`io.tangencylabs`), not part of this suite.

When a new version ships, re-extract the CLI from the AppImage
(`usr/bin/<app>-cli`) into `~/.local/share/<app>/<version>/` and repoint the
`~/.local/bin/<app>-cli` symlink. Installed versions are recorded in
`~/.local/share/artcraft-launcher/installed.json`.
