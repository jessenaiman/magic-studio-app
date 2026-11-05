#!/usr/bin/env bash
# One character through Tripo Studio: T-pose image -> HD model -> humanoid auto-rig -> presets -> FBX (Blender) zip.
# usage: tripo_character.sh NAME IMAGE OUTDIR
# Uses the saved Geometry & Texture settings (Ultra off, Texture on, Quad, 10000). Paced to avoid bot checks.
set -u
N=$1; IMG=$2; OUT=$3; mkdir -p "$OUT"
export AGENT_BROWSER_SESSION=tripo
ab(){ agent-browser --profile Default --headed "$@"; sleep 3; }
snap(){ agent-browser --profile Default --headed snapshot -i -c 2>&1; }
ref(){ snap | grep -o "$1[^\[]*\[[^]]*ref=e[0-9]*" | head -1 | grep -o 'e[0-9]*$'; }
waitgone(){ for k in $(seq 1 60); do sleep 10; snap | grep -q -i -E "$1" || return 0; done; }
log(){ echo "[$N] $*"; }

# 1. Model: single image
ab press Escape >/dev/null
ab click @$(ref 'link "Model"') >/dev/null
ab click @$(ref 'button "HD Model"') >/dev/null
first=$(snap | sed -n '/button "Smart Mesh"/,$p' | grep -o 'button \[ref=e[0-9]*' | head -1 | grep -o 'e[0-9]*$')
ab click @$first >/dev/null
agent-browser --profile Default --headed eval "[...document.querySelectorAll('[class*=\"i-tripo:delete\"]')].filter(e=>e.getBoundingClientRect().x<330).forEach(d=>(d.closest('button')||d).click())" >/dev/null; sleep 2
ab upload 'input[type=file]' "$IMG" >/dev/null
before=$(snap | grep -o 'link "[^"]*3d model [0-9-]* [0-9:]*"' | head -1)
ab click @$(ref 'button "Generate [0-9]') >/dev/null
log "generating model"
for k in $(seq 1 60); do sleep 10; now=$(snap | grep -o 'link "[^"]*3d model [0-9-]* [0-9:]*"' | head -1); [ -n "$now" ] && [ "$now" != "$before" ] && ! snap | grep -q -i 'generating' && break; done
log "model: $now"
ab click @$(ref "$now") >/dev/null

# 2. Rig: humanoid auto-rig
ab click @$(ref 'link "Rig"') >/dev/null
h=$(snap | grep -o 'radio "Humanoid Humanoid" \[checked=false, ref=e[0-9]*' | grep -o 'e[0-9]*$'); [ -n "$h" ] && ab click @$h >/dev/null
ab click @$(ref 'button "Auto Rig [0-9]') >/dev/null
for k in $(seq 1 60); do sleep 10; s=$(snap); echo "$s" | grep -q -E 'Retry [0-9]+|Rigged models' && break; echo "$s" | grep -q -i -E 'exceed|failed' && { log "RIG FAILED"; exit 1; }; done
log "rigged"

# 3. Animate: free presets
ab click @$(ref 'link "Animate"') >/dev/null
pt=$(snap | grep -o 'tab "Preset"[^\[]*\[[^]]*ref=e[0-9]*' | grep -o 'e[0-9]*$' | head -1); [ -n "$pt" ] && ab click @$pt >/dev/null
s=$(ref 'textbox "Search"')
for q in idle walk wave_goodbye_01 jump cheer; do
  ab fill @$s "$q" >/dev/null; sleep 2
  r=$(snap | grep -o "button \"$q $q\" \[ref=e[0-9]*" | head -1 | grep -o 'e[0-9]*$'); [ -n "$r" ] && ab click @$r >/dev/null && log "preset $q"
done

# 4. Export FBX for Blender with every animation
ab click @$(ref 'button "Export"') >/dev/null
ab click @$(ref 'button "Blender"') >/dev/null
dd=$(snap | sed -n '/combobox.*FBX/,/Send to/p' | grep -o 'button "[0-9]*" \[expanded=false, ref=e[0-9]*' | grep -o 'e[0-9]*$'); ab click @$dd >/dev/null
sa=$(snap | grep -o 'checkbox "Select All" \[checked=false, ref=e[0-9]*' | grep -o 'e[0-9]*$'); [ -n "$sa" ] && ab click @$sa >/dev/null
ab press Escape >/dev/null
bt=$(snap | sed -n '/Send to/,$p' | grep -o 'button "Export" \[ref=e[0-9]*' | head -1 | grep -o 'e[0-9]*$')
for t in 1 2 3; do timeout 400 agent-browser --profile Default --headed download @$bt "$OUT/$N.zip" >/dev/null 2>&1 && break; sleep 15; done
[ -s "$OUT/$N.zip" ] && (cd "$OUT" && unzip -oq "$N.zip") && log "exported $(ls "$OUT"/*.fbx)" || log "EXPORT FAILED"
