# Review background for Magic Studio

Magic Studio makes music videos by chaining installed desktop apps. Review every change against
these rules and report any violation as a finding.

- The apps do the work. Images, layers, masks, motion, rigs, renders and edits go through the
  installed apps' own CLI or MCP commands (PhotoCraft, VectorCraft, EffectCraft, FilmCraft,
  DeckCraft, Codex image generation as "Scribbles"). Code in this repository only chains those
  calls through `magic-cli`. Flag any code that computes pixels, curves, keyframes, IK, matting or
  layout itself instead of asking an app.
- No scraping of human-readable tool output when the tool offers a machine format (JSON, SARIF).
- No pinned app or runtime versions in code or config; resolve installed apps and tools at run
  time.
- No Python, no Blender path, no in-browser animation engine; the Studio UI shows what the apps
  rendered.
- A command must verify what it claims (read back the app's result) and fail loudly when the app
  did not do the work; flag silent no-ops and success messages without evidence.
- Lint rules are loosened (limits raised), never disabled; Rust stays `cargo fmt` clean.
- Flag sprawl: functions over 100 lines or cyclomatic/cognitive complexity over 15.
