# Fidget → Scribbles

Fidget owns animation drawing templates and composes the final prompt.
Scribbles executes that prompt with the existing image backend. Preparation
is deterministic; it does not launch Fidget's chat agent or an image job.

The two commands are registered in Rust and available through magic-cli
and its MCP server. Existing `scribbles.new`, `image`, `sheet`, `poses`,
`actions`, and `custom` commands prepare through Fidget then generate through
Scribbles. Existing `sprocket.*` chat commands are unchanged.

## Proposed CLI workflow

Run against a build containing these commands. An older installed desktop
cannot dispatch new command IDs. Use a separate session binding while
reviewing a source build, or install the reviewed build before attaching
its desktop window.

```bash
magic-cli fidget prepare kind=actions action=walk \
  reference=assets/hopper.png prompt='Hopper, preserving the reference' \
  > prepared.json

# Inspect the final prompt and ordered references before generation.
jq '{prompt, references, fidget}' prepared.json

magic-cli --project /absolute/Farm.magic scribbles generate \
  --params "$(cat prepared.json)"
```

Preparation returns a flat object: `prompt`, optional ordered `references`,
`transparent_background`, `timeout_s`, `name`, supplied provider/model and
project/character identifiers, plus `ok` and `fidget` provenance. Pass the
entire object to generation. Top-level project selection follows existing
CLI routing; a nested request envelope is not supported.

References accept the legacy `reference` string/array or `references`
string/array, never both. Preparation rejects non-string/empty paths and
more than five paths. New/custom requests may omit references. Generation
then verifies file existence, PNG decoding, and containment in the selected
project directory using the existing image backend. Preparation alone does
not establish that reference files are usable.

Scribbles does not add pose/rig text: `poses` and `rig_parts` parameters are
rejected. Put their instructions in the final prompt. Provider transport
instructions still direct Codex to invoke its native image-generation tool.

## Settings

Existing settings.json can contain:

```json
{"scribbles": {"provider": "codex", "model": "gpt-6-luna"}}
```

Provider precedence: request → scribbles.provider → existing image backend
resolution (MAGICSTUDIO_IMAGE_PROVIDER → image.provider → codex).
Model precedence: request → scribbles.model → the drawing template default
(currently gpt-6-luna). This preserves the existing Scribbles model default;
image backend environment/model settings do not override it.

Provider/model values must be nonempty strings. The backend currently
supports codex and configured http; new providers require a backend adapter.
Luna is the Codex agent model, not a separate image provider. HTTP endpoint
and credential environment-variable configuration remains owned by image
settings; credentials never belong in prepared requests.

## Review and verification

Compiler, formatter and Clippy checks cover this change. Existing prompt
unit tests moved with their implementation; no new behavioral tests were
added, per CONTRIBUTING.md's user-observation requirement.

Proposed observations, pending Jesse's UI review and approval:

- Prepare new/custom without references; inspect prompt without generation.
- Prepare a referenced action sheet; confirm ordered references and canvas.
- Reject malformed, excessive or conflicting reference fields.
- Generate the prepared request in a disposable project; compare saved
  prompt, image receipt, reference hashes, dimensions and returned metadata.
- Repeat through an existing Scribbles UI command; confirm compatibility.
- Confirm provider/model selection and errors for invalid settings.
- Confirm concurrent desktop edits cannot race Scribbles asset saves.

Scribbles generation uses the desktop's serialized image-generation path.
This protects project writes but can delay commands requiring the same
engine lock for the duration of a generation job.
