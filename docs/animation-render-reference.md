# Animation Render Reference — Magic Studio

Single source of truth for rendering animation clips through EffectCraft (`character.render` / `effectcraft-cli`). No Blender — EffectCraft is the render authority for all 2D, 2.5D and 3D animation in Magic Studio.

## 1. EffectCraft render via `character.render`

The Craft-native render command. Discovers the `.ecproj`, comp and frame range, then calls `effectcraft-cli render`.

```bash
target/debug/magic-cli --project <project.magic> exec character.render \
  effectcraft_project=<path/to/file.ecproj> \
  comp=<comp_name_or_id> \
  start=<start_seconds> \
  end=<end_seconds> \
  fps=<fps> \
  format=frames
```

Parameters:
- `effectcraft_project` — absolute path to the `.ecproj` file
- `comp` — composition name (string) or ID (number) inside the ecproj. Find comps by inspecting the `.ecproj` JSON: items where `kind.type == "Comp"`
- `start` / `end` — time range in seconds (`end` must be > `start`)
- `fps` — frame rate, clamped 1-60
- `format` — `frames` (PNG sequence), `clip` (ProRes 4444 MOV with alpha), or `gif`

Output (PNG frames): `projects/<proj>/film/frames/<ecproj_stem>-<timestamp>/frame_XXXX.png`

### Finding comp names/IDs

```bash
python3 -c "
import json
with open('<file>.ecproj') as f:
    d = json.load(f)
for k, v in d['items'].items():
    if v.get('kind', {}).get('type') == 'Comp':
        print(f'id={k} name={v[\"name\"]!r}')
"
```

### Transparency

EffectCraft renders PNG sequences with RGBA channels (`--channels rgba`). All frames carry alpha — no separate transparency setting needed (unlike Blender's `film_transparent`).

## 2. EffectCraft CLI directly

For one-off renders outside Magic Studio:

```bash
effectcraft-cli --json --project <file.ecproj> render \
  --comp <name|id> \
  --out <output.png> \
  --start <seconds> \
  --end <seconds> \
  --fps <fps> \
  --format png \
  --channels rgba
```

## 3. Review statuses (Frame.io / CGWire / Director's Cut patterns)

| Status | Meaning |
|---|---|
| `needs-visual-review` | Rendered, awaiting human playthrough |
| `approved` | Cleared for use, tied to this version |
| `issues-found` | Specific corrections needed (note exact frames/time ranges) |

Each revision entry in `animation-review.json` carries `trim_start`/`trim_end`, `speed`, `loop`, `in_place`, `notes`, `status`, and `source_sha256`.

Revision protocol: `expected_revision` must equal the current revision on disk. Each `save_revision` increments by 1.

## 4. Clip inventory — `Just a Few Minutes of Magic.magic`

### 2D / 2.5D sprite cycles (rendered via EffectCraft, contact sheets generated)

| Rev | Character | Clip | Source ecproj | Comp | Frames | FPS | Duration | Style | Status |
|---|---|---|---|---|---|---|---|---|---|
| 26 | fan_moth | `fan_moth-front-0.00-2.00` | `assets/characters/fan-moth/fan-moth.ecproj` | 1 | 32 | 24 | 1.33s | cutout_2_5d | needs-visual-review |
| 27 | oilcan_badger | `oilcan_badger-front-0.00-2.00` | `assets/characters/oilcan-badger/oilcan-badger-rig.ecproj` | 1 | 32 | 24 | 1.33s | cutout_2_5d | needs-visual-review |
| 28 | label_pangolin | `walk_right` | `assets/characters/label_pangolin/sprites/walk_right/label_pangolin-walk_right.ecproj` | 2 | 8 | 8 | 1.0s | sprite_2d | needs-visual-review |
| 29 | label_pangolin | `walk_down` | `assets/characters/label_pangolin/sprites/walk_down/label_pangolin-walk_down.ecproj` | 2 | 24 | 24 | 1.0s | cutout_2_5d | needs-visual-review |
| 30 | toasty | `wave` | `assets/characters/toasty/front/toasty-front.ecproj` | 2 | 48 | 24 | 2.0s | cutout_2_5d | needs-visual-review |
| 31 | toasty | `idle` | `assets/characters/toasty/front/toasty-front.ecproj` | 2 | 48 | 24 | 2.0s | cutout_2_5d | needs-visual-review |

### 3D rig_3d clips — source animations NOT available

| Rev | Character | Clip | Issue |
|---|---|---|---|
| 3-5 | label_pangolin | idle, jump, walk_bounce | GLB exists (`label_pangolin-moppet-rig.glb`) with 3 animations, but no EffectCraft ecproj set up to render them through `character.render` |
| 22-25 | toasty | `Armature\|wave`, `wave_goodbye_01`, `blender_action_1`, `blender_action_2` | No matching actions in `toasty-spring.blend` (only `Toasty spring rigAction`, spring physics rig). External anim JSONs reference `/run/media/jesse/Speedy/magic/` paths that don't exist. Marked `issues-found`. |

## 5. Output locations

- EffectCraft renders: `projects/<proj>/film/frames/<ecproj_stem>-<timestamp>/frame_XXXX.png`
- Contact sheets: `projects/<proj>/review-contact-sheets/<char>-<clip>-effectcraft.png`
- Review log: `projects/<proj>/animation-review.json`
- Source .ecproj files: `assets/characters/<char>/<subdir>/<name>.ecproj`
