#!/usr/bin/env bash
# Layered 2D cut-out animation, built only from Craft CLI calls. Overlapping video tracks provide the
# picture and hide every joint.
#
# Reviewers, follow these:
#   Design tutorial: parts, joints and hierarchy (each section its own layer, circles define the
#   overlap, hooks at joint centres, parent first):
#     https://tahoma2d.readthedocs.io/en/latest/creating_cutout_animation.html
#   Video, seamless joints in layered characters (Lee Daniels, Quick Tip, 2018):
#     https://lesterbanks.com/2018/10/2-ways-create-seamless-joints-ae/
#   Video, depth from planes that travel at different speeds (Walt Disney, "Tricks of Our Trade", 1957):
#     https://www.youtube.com/watch?v=kN-eCBAOw60
#   Hinged cut-outs with concealed joints (Lotte Reiniger, 1936):
#     https://www.bfi.org.uk/news-opinion/sight-sound-magazine/archives/lotte-reiniger-scissors-make-films-cut-out-animation-process-description-1936
#   Vector joints and joint markers (Jake Bartlett):
#     https://www.skillshare.com/en/classes/character-rigging-with-duik-bassel/1265117639
#
# The rules:
#   1. Every part is drawn whole, past its joint, ending in a round cap. The overlap is in the art.
#   2. The pivot is the centre of that cap, so a rotating part never shows a gap.
#   3. Stacking hides the seam: the part that reads as continuous sits above the cap it covers.
#   4. Parent first: torso, then head, upper limbs, then forearms.
#   5. Each plane and part is its own video track; near planes travel further than far ones.
#
# Input in $SRC: layers-sheet.png (3 x 3 cells of 418 px: head, torso, near upper arm; near forearm,
# far upper arm, far forearm; near leg, far leg), plane-back.png, plane-mid.png, plane-fore.png.
# Needs photocraft-cli, effectcraft-cli, filmcraft-cli and jq.
set -euo pipefail

ROOT="${ROOT:-$PWD/projects/craft-proof}"
SRC="$ROOT/source"
OUT="$ROOT/layered"
FPS=24
SECONDS_LONG=6
CELL=418
mkdir -p "$OUT/parts" "$OUT/tracks"

# PhotoCraft cuts each part. Canvas stays the full sheet, so pivots below are sheet coordinates.
cut_part() {
  photocraft-cli run "$SRC/layers-sheet.png" \
    --cmd layer.matting.defringe --params '{"width":2}' \
    --cmd select.rect --params "{\"x\":$(($2 * CELL)),\"y\":$(($3 * CELL)),\"width\":$CELL,\"height\":$CELL}" \
    --cmd select.inverse --cmd edit.clear --cmd select.deselect \
    --out "$OUT/parts/$1.png" >/dev/null
}
cut_part head 0 0; cut_part torso 1 0; cut_part near_upper 2 0
cut_part near_fore 0 1; cut_part far_upper 1 1; cut_part far_fore 2 1
cut_part near_leg 0 2; cut_part far_leg 1 2

key() { jq -nc --arg l "$1" --arg p "$2" --argjson t "$3" --argjson v "$4" '{command:"prop.addKey",params:{layer:$l,path:("transform/"+$p),time:$t,timeBase:"comp",value:$v}}'; }
set_tf() { jq -nc --arg l "$1" --arg p "$2" --argjson v "$3" '{command:"layer.setTransform",params:{layers:[$l],prop:$p,value:$v}}'; }
add() { jq -nc --arg f "$2" --arg n "$1" '[{command:"file.import",params:{paths:[$f]}},{command:"layer.addItem",params:{item:($f|split("/")|last)}},{command:"layer.rename",params:{name:$n}}][]'; }
parent() { jq -nc --arg l "$1" --arg p "$2" '{command:"layer.setParent",params:{layers:[$l],parent:$p,compensate:true}}'; }
place() { set_tf "$1" scale "[$2,$2]"; set_tf "$1" anchor "$3"; set_tf "$1" position "$4"; }

# Bottom of the stack first.
TRACKS=(back mid far_leg near_leg head torso far_fore far_upper near_fore near_upper front)
{
  jq -nc --argjson fps "$FPS" --argjson d "$SECONDS_LONG" '{command:"comp.new",params:{name:"layered",width:1280,height:720,frameRate:$fps,duration:$d}}'
  add back "$SRC/plane-back.png"; add mid "$SRC/plane-mid.png"
  for part in far_leg near_leg head torso far_fore far_upper near_fore near_upper; do add "$part" "$OUT/parts/$part.png"; done
  add front "$SRC/plane-fore.png"

  place torso 62 '[627,190]' '[640,350]'
  place head 60 '[225,365]' '[640,257]'
  place near_upper 50 '[1038,78]' '[693,292]'
  place near_fore 50 '[220,464]' '[692,401]'
  place far_upper 50 '[627,490]' '[588,292]'
  place far_fore 50 '[1035,463]' '[588,400]'
  place near_leg 62 '[222,857]' '[689,450]'
  place far_leg 62 '[626,857]' '[591,450]'
  parent head torso; parent near_upper torso; parent far_upper torso; parent near_leg torso; parent far_leg torso
  parent near_fore near_upper; parent far_fore far_upper

  # Multiplane travel: the nearer the plane, the further it moves.
  key back position 0 '[690,250]'; key back position "$SECONDS_LONG" '[590,250]'
  key mid position 0 '[720,250]'; key mid position "$SECONDS_LONG" '[560,250]'
  key front position 0 '[790,250]'; key front position "$SECONDS_LONG" '[490,250]'

  # The wave reaches extreme joint angles, so any gap would show.
  key near_upper rotation 0 0; key near_upper rotation 1 -125; key near_upper rotation 5 -125; key near_upper rotation 6 0
  key head rotation 0 -6; key head rotation 3 6; key head rotation 6 -6
  key far_fore rotation 0 10; key far_fore rotation 3 -20; key far_fore rotation 6 10
  for t in 0 1 2 3 4 5 6; do
    key near_fore rotation "$t" -35; key far_upper rotation "$t" 32
    key near_leg rotation "$t" 16; key far_leg rotation "$t" -16; key torso position "$t" '[640,350]'
  done
  for t in 0.5 1.5 2.5 3.5 4.5 5.5; do
    key far_upper rotation "$t" 12; key near_leg rotation "$t" -16; key far_leg rotation "$t" 16; key torso position "$t" '[640,340]'
  done
  for t in 1.5 2.5 3.5 4.5; do key near_fore rotation "$t" 35; done
} > "$OUT/rig.jsonl"
effectcraft-cli --empty --save-as "$OUT/rig.ecproj" run --commands "$OUT/rig.jsonl" >/dev/null

# One video track per layer: everything else hidden, rendered with alpha.
for keep in "${TRACKS[@]}"; do
  for other in "${TRACKS[@]}"; do [ "$other" = "$keep" ] || set_tf "$other" opacity 0; done > "$OUT/tracks/$keep.jsonl"
  effectcraft-cli --project "$OUT/rig.ecproj" --save-as "$OUT/tracks/$keep.ecproj" run --commands "$OUT/tracks/$keep.jsonl" >/dev/null
  effectcraft-cli --project "$OUT/tracks/$keep.ecproj" render --comp layered --out "$OUT/tracks/$keep.mov" --format prores --prores 4444 --channels rgba --fps "$FPS" >/dev/null
done

# FilmCraft stacks the overlapping tracks and exports.
FILM="$OUT/layered.fcproj"
{
  jq -nc --argjson fps "$FPS" --argjson n "${#TRACKS[@]}" '{id:"file.newSequence",params:{name:"Layered",width:1280,height:720,fps:$fps,video:$n,audio:1}}'
  for track in "${TRACKS[@]}"; do jq -nc --arg f "$OUT/tracks/$track.mov" '{id:"file.import",params:{paths:[$f]}}'; done
} > "$OUT/film.jsonl"
items=$(filmcraft-cli --save-as "$FILM" --compact run "$OUT/film.jsonl" | jq -s -c '[.[] | (.result // .) | select(.items) | .items[0]]')
n=0
for _ in "${TRACKS[@]}"; do
  jq -nc --argjson i "$(jq ".[$n]" <<<"$items")" --arg t "V$((n + 1))" '{id:"timeline.place",params:{item:$i,track:$t,seconds:0,insert:false,audio:false}}'
  n=$((n + 1))
done > "$OUT/place.jsonl"
filmcraft-cli --project "$FILM" --save --compact run "$OUT/place.jsonl" >/dev/null
filmcraft-cli --project "$FILM" export "$OUT/layered.mp4" --format h264 >/dev/null
echo "$OUT/layered.mp4"
