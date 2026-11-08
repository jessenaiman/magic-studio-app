#!/usr/bin/env bash
# Proof video: layered 2D animation and fast sprite animation, using only Craft and Magic CLI calls.
#
# PART 1. LAYERED 2D (cut-out animation composited as overlapping video tracks). Reviewers: follow these.
#   Design tutorial, parts, joints and hierarchy (each section its own layer, circles define the
#   overlap, hooks at joint centres, parent first):
#     https://tahoma2d.readthedocs.io/en/latest/creating_cutout_animation.html
#   Video, seamless joints in layered characters (Lee Daniels, Quick Tip, 2018):
#     https://lesterbanks.com/2018/10/2-ways-create-seamless-joints-ae/
#   Video, depth from planes that travel at different speeds (Walt Disney, "Tricks of Our Trade", 1957):
#     https://www.youtube.com/watch?v=kN-eCBAOw60
#     Source that links it: https://www.wdwradio.com/2016/02/awesome-facts-can-learn-studying-walt-disneys-multiplane-camera/
#   History of hinged cut-outs with concealed joints (Lotte Reiniger, 1936):
#     https://www.bfi.org.uk/news-opinion/sight-sound-magazine/archives/lotte-reiniger-scissors-make-films-cut-out-animation-process-description-1936
#
#   The rules this script follows:
#     1. Every part is drawn whole, past its joint, ending in a round cap (the overlap is in the art).
#     2. The pivot is the centre of that cap, and neighbouring caps share it, so rotation shows no gap.
#     3. Stacking hides the seam: the part that should read as continuous sits above the cap it covers.
#     4. Parent first: body, then head and upper arm, then forearm.
#     5. Each plane and part is its own video track. Far planes travel less than near planes.
#
# PART 2. SPRITE ANIMATION. Scribbles draws one movement sheet from a single image; PhotoCraft divides
#   it; the frames chosen in SELECT are placed on a final sheet by PhotoCraft; EffectCraft plays them
#   at 24 fps, every drawing held for one frame, not the usual 12.
#     Timing reference (on ones and twos): https://publish.obsidian.md/mister-chad/animation/animating+on+the+ones%2C+two%2C+and+threes
#
# Needs: photocraft-cli, effectcraft-cli, filmcraft-cli, magic-cli, jq.
set -euo pipefail

ROOT="${ROOT:-$PWD/projects/craft-proof}"
SRC="$ROOT/source"
OUT="$ROOT/proof"
MAGIC="$ROOT/Craft Proof.magic"
MC="${MC:-magic-cli}"
SELECT="${SELECT:-1 2 3 4 5 6 7 8}"
FPS=24
LAYERED_SECONDS=6
SPRITE_SECONDS=4
mkdir -p "$OUT/parts" "$OUT/tracks"

# ---- Part 1: PhotoCraft divides the parts sheet. Pivots below are measured in cell coordinates.
CELL=627
crop() { photocraft-cli run "$SRC/parts-sheet.png" --cmd image.crop --params "{\"x\":$2,\"y\":$3,\"width\":$CELL,\"height\":$CELL}" --out "$OUT/parts/$1.png" >/dev/null; }
crop head 0 0
crop body "$CELL" 0
crop upper 0 "$CELL"
crop fore "$CELL" "$CELL"

# ---- EffectCraft builds the rig: planes and parts, each its own layer.
key() { jq -nc --arg l "$1" --arg p "$2" --argjson t "$3" --argjson v "$4" '{command:"prop.addKey",params:{layer:$l,path:("transform/"+$p),time:$t,timeBase:"comp",value:$v}}'; }
transform() { jq -nc --arg l "$1" --arg p "$2" --argjson v "$3" '{command:"layer.setTransform",params:{layers:[$l],prop:$p,value:$v}}'; }
add() { jq -nc --arg f "$2" --arg n "$1" '[{command:"file.import",params:{paths:[$f]}},{command:"layer.addItem",params:{item:($f|split("/")|last)}},{command:"layer.rename",params:{name:$n}}][]'; }
parent() { jq -nc --arg l "$1" --arg p "$2" '{command:"layer.setParent",params:{layers:[$l],parent:$p,compensate:true}}'; }
{
  jq -nc --argjson fps "$FPS" --argjson d "$LAYERED_SECONDS" '{command:"comp.new",params:{name:"proof",width:1280,height:720,frameRate:$fps,duration:$d}}'
  # Bottom of the stack first.
  add back "$SRC/plane-back.png"; add mid "$SRC/plane-mid.png"
  add head "$OUT/parts/head.png"; add body "$OUT/parts/body.png"
  add upper "$OUT/parts/upper.png"; add fore "$OUT/parts/fore.png"
  add front "$SRC/plane-fore.png"

  # Rest pose. Anchors are the cap centres requested in the parts sheet.
  for part in head body upper fore; do transform "$part" scale '[56,56]'; done
  transform body anchor '[308,605]'; transform body position '[640,690]'
  transform head anchor '[307,482]'; transform head position '[634,399]'
  transform upper anchor '[311,128]'; transform upper position '[735,430]'
  transform fore anchor '[312,125]'; transform fore position '[736,630]'
  parent head body; parent upper body; parent fore upper

  # Multiplane travel: the nearer the plane, the further it moves.
  key back position 0 '[690,250]'; key back position "$LAYERED_SECONDS" '[590,250]'
  key mid position 0 '[720,250]'; key mid position "$LAYERED_SECONDS" '[560,250]'
  key front position 0 '[790,250]'; key front position "$LAYERED_SECONDS" '[490,250]'

  # The wave: extreme joint angles, so any gap would show.
  key upper rotation 0 0; key upper rotation 1 -125; key upper rotation 5 -125; key upper rotation 6 0
  for t in 1 2 3 4 5; do key fore rotation "$t" -40; done
  for t in 1.5 2.5 3.5 4.5; do key fore rotation "$t" 35; done
  key fore rotation 0 0; key fore rotation 6 0
  key head rotation 0 -5; key head rotation 3 6; key head rotation 6 -5
  key body position 0 '[640,690]'; key body position 1.5 '[640,682]'; key body position 3 '[640,690]'; key body position 4.5 '[640,682]'; key body position 6 '[640,690]'
} > "$OUT/rig.jsonl"
effectcraft-cli --empty --save-as "$OUT/rig.ecproj" run --commands "$OUT/rig.jsonl" >/dev/null

# ---- One video track per plane or part: everything else hidden, rendered with alpha.
TRACKS=(back mid head body upper fore front)
for keep in "${TRACKS[@]}"; do
  for other in "${TRACKS[@]}"; do [ "$other" = "$keep" ] || transform "$other" opacity 0; done > "$OUT/tracks/$keep.jsonl"
  effectcraft-cli --project "$OUT/rig.ecproj" --save-as "$OUT/tracks/$keep.ecproj" run --commands "$OUT/tracks/$keep.jsonl" >/dev/null
  effectcraft-cli --project "$OUT/tracks/$keep.ecproj" render --comp proof --out "$OUT/tracks/$keep.mov" --format prores --prores 4444 --channels rgba --fps "$FPS" >/dev/null
done

# ---- Part 2: Scribbles' movement sheet, divided by PhotoCraft and played by EffectCraft.
sprite=$("$MC" --project "$MAGIC" exec character.sprite character=toasty action=run sheet="$SRC/run-sheet.png" columns=4 rows=2 fps="$FPS" duration="$SPRITE_SECONDS")
dir=$(jq -r '.sprite.sheet | rtrimstr("/sheet.png")' <<<"$sprite")

# The chosen frames go on a final sheet, placed by PhotoCraft on a 384 x 512 grid.
args=(); index=0
for frame in $SELECT; do
  center=$(jq -nc --argjson i "$index" '[($i%4)*384+192, (($i/4)|floor)*512+256]')
  args+=(--cmd file.placeEmbedded --params "$(jq -nc --arg p "$dir/frames/frame_$(printf %04d "$frame").png" --argjson c "$center" '{path:$p,center:$c,scale:100,fit:false,smartObject:false}')")
  index=$((index + 1))
done
photocraft-cli run --new '{"width":1536,"height":1024,"background":"transparent"}' "${args[@]}" --out "$OUT/final-sheet.png" >/dev/null

cycle=$(jq -r '.sprite.cycle_seconds' <<<"$sprite")
clip=$("$MC" --project "$MAGIC" exec character.render character=toasty sprite=run start=0 end="$SPRITE_SECONDS" fps="$FPS" format=clip | jq -r .clip)

# ---- FilmCraft stacks the overlapping tracks and exports the proof.
FILM="$OUT/proof.fcproj"
import() { jq -nc --arg f "$1" '{id:"file.import",params:{paths:[$f]}}'; }
{
  jq -nc --argjson fps "$FPS" '{id:"file.newSequence",params:{name:"Proof",width:1280,height:720,fps:$fps,video:7,audio:1}}'
  for track in "${TRACKS[@]}"; do import "$OUT/tracks/$track.mov"; done
  import "$clip"
} > "$OUT/film.jsonl"
items=$(filmcraft-cli --save-as "$FILM" --compact run "$OUT/film.jsonl" | jq -s -c '[.[] | (.result // .) | select(.items) | .items[0]]')

place() { jq -nc --argjson i "$1" --arg t "$2" --argjson s "$3" '{id:"timeline.place",params:{item:$i,track:$t,seconds:$s,insert:false,audio:false}}'; }
{
  n=0
  for track in "${TRACKS[@]}"; do place "$(jq ".[$n]" <<<"$items")" "V$((n + 1))" 0; n=$((n + 1)); done
  # Part 2 runs after Part 1, over the same far planes.
  place "$(jq '.[0]' <<<"$items")" V1 "$LAYERED_SECONDS"
  place "$(jq '.[1]' <<<"$items")" V2 "$LAYERED_SECONDS"
  place "$(jq '.[7]' <<<"$items")" V3 "$LAYERED_SECONDS"
} > "$OUT/place.jsonl"
filmcraft-cli --project "$FILM" --save --compact run "$OUT/place.jsonl" >/dev/null
filmcraft-cli --project "$FILM" export "$OUT/proof.mp4" --format h264 --allow-offline >/dev/null

echo "proof: $OUT/proof.mp4"
echo "final sprite sheet: $OUT/final-sheet.png (cycle ${cycle}s at ${FPS} fps)"
